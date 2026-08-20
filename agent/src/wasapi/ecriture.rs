//! Le client de rendu WASAPI qui **écrit** le micro sur le câble virtuel.
//!
//! Miroir exact de `LoopbackCapture::open` (`agent/src/wasapi.rs`) : même
//! contrôle d'appartement COM, même garde RAII sur le `WAVEFORMATEX`, même
//! refus explicite d'un format inattendu. Trois choses seulement changent, et
//! ce sont les trois seules qui soient neuves ici :
//!
//! - `AUDCLNT_STREAMFLAGS_LOOPBACK` disparaît ;
//! - `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` + `SetEventHandle` apparaissent —
//!   **tentés, et non supposés** (voir [`Reveil`]) ;
//! - `IAudioCaptureClient::GetBuffer` devient `IAudioRenderClient::GetBuffer`
//!   suivi de `ReleaseBuffer(trames, 0)`, et la place disponible se calcule
//!   `GetBufferSize() - GetCurrentPadding()`.
//!
//! ## Ce module n'est PAS `Send`, et c'est délibéré
//!
//! `LoopbackCapture` porte un `unsafe impl Send` dont le commentaire nomme
//! lui-même ce qui l'invaliderait : « si un futur champ ajoute un `HANDLE`
//! d'événement […] cet `unsafe impl` cesserait d'être valide sans que rien ne
//! le signale ». `RenduWasapi` a exactement ce champ. Plutôt que d'écrire
//! l'argument qui sauverait le `Send` — le `HANDLE` d'événement est lié au
//! processus, pas à l'appartement —, on **ouvre sur le fil de rendu
//! lui-même** : aucun objet COM ne traverse de frontière de fil, il n'y a donc
//! aucune promesse à tenir. C'est le parti le plus simple et le seul sans
//! dette (plan E2, tâche 7).
//!
//! La contrepartie est que `windows_micro::ouvrir` ne peut pas connaître le
//! verdict d'ouverture en revenant d'un `spawn` : il l'apprend par un canal.
//! Voir ce module.
//!
//! ## Ce module ne se teste PAS sur l'hôte
//!
//! `#![cfg(windows)]`. Tout ce qui peut se tromper en est sorti : la
//! désignation du câble (`wasapi/peripherique.rs`), l'exclusivité
//! (`micro/exclusivite.rs`), la garde de boucle (`micro/boucle_locale.rs`) et
//! **le contrôle de format** (`wasapi/format.rs`), tous purs et éprouvés sous
//! Linux. Ce qui reste ici est de la plomberie COM que seule une recette sur
//! la VM éprouve.

#![cfg(windows)]

use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, WAIT_FAILED};
use windows::Win32::Media::Audio::{
    IAudioClient, IAudioRenderClient, IMMDevice, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use crate::wasapi_format;

/// Durée du tampon de rendu demandé à WASAPI, en unités de 100 ns. 40 ms.
///
/// ⚠️ **NON CALIBRÉE**, et elle rejoint la liste que `CLAUDE.md` tient
/// (`BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `TAILLE_MAX_SORTIE`…).
/// Ce n'est pas une durée neutre : en régime établi le tampon reste **plein**
/// — `remplir` ne bloque jamais et complète au silence, donc toute la place
/// rendue par `attendre_place` est consommée à chaque tour —, et cette durée
/// est donc **le plancher de latence que l'écriture ajoute**, au-dessus de
/// celle du tampon de gigue. 40 ms est un compromis écrit plutôt que mesuré :
/// assez pour absorber quatre périodes du moteur audio partagé (10 ms est sa
/// granularité usuelle sur cette VM, relevée au chantier A), assez peu pour ne
/// pas doubler la latence de bout en bout. **Aucun jugement d'écoute n'a été
/// porté.**
const DUREE_TAMPON_100NS: i64 = 400_000;

/// Étiquette `WAVE_FORMAT_EXTENSIBLE` du champ `wFormatTag`. Même constante
/// que dans `wasapi.rs` ; les deux moitiés du son lisent le même champ.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Étiquette `WAVE_FORMAT_IEEE_FLOAT`.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// Comment le fil de rendu est réveillé. **MESURÉ, jamais supposé.**
///
/// La spec §6 affirme que `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` « fonctionne
/// ici, car il s'agit d'un flux de rendu ordinaire et non d'un loopback ».
/// C'est une **prédiction** : aucun code de ce dépôt n'avait jamais ouvert un
/// flux de rendu WASAPI avant ce bloc. On le tente donc, on se replie sur une
/// boucle à échéance si `Initialize` le refuse, **et le mode réellement obtenu
/// est journalisé** — faute de quoi une recette ne saurait pas ce qu'elle
/// mesure (Décision 8 du plan E2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveil {
    /// `SetEventHandle` a été accepté : WASAPI signale l'événement à chaque
    /// période.
    Evenement,
    /// Repli : on sonde `GetCurrentPadding` à échéance. C'est le mode que le
    /// fil de mesure de E1 emploie déjà (`demarrage/micro/mesure.rs`).
    Echeance,
}

impl Reveil {
    /// Le libellé qui part au journal.
    pub fn libelle(self) -> &'static str {
        match self {
            Reveil::Evenement => "evenement",
            Reveil::Echeance => "echeance",
        }
    }
}

/// Garde RAII pour le pointeur rendu par `IAudioClient::GetMixFormat`.
///
/// Jumeau de `wasapi::FormatMixage`, dupliqué plutôt que partagé : celui-là
/// est privé à son module, et le hisser exigerait de rendre `wasapi.rs`
/// dépendant d'un module qui dépend de lui. Sept lignes contre un cycle.
/// **La raison du garde est la même** : entre `GetMixFormat` et `Initialize`,
/// l'ouverture peut sortir par plusieurs `?` — exactement les chemins qu'on
/// emprunte le jour où la machine change de configuration audio — et une
/// libération posée en fin de fonction heureuse les manquerait.
struct FormatMixage(*mut WAVEFORMATEX);

impl std::ops::Deref for FormatMixage {
    type Target = WAVEFORMATEX;
    fn deref(&self) -> &WAVEFORMATEX {
        // SAFETY : le pointeur vient d'un `GetMixFormat` réussi et n'est
        // libéré que dans `Drop`.
        unsafe { &*self.0 }
    }
}

impl Drop for FormatMixage {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

/// Rejoint l'appartement multi-thread, ou refuse.
///
/// À appeler **sur le fil de rendu**, avant tout appel COM. Copié du contrôle
/// de `LoopbackCapture::open` et pour la même raison : `S_OK` (le fil vient de
/// rejoindre la MTA) et `S_FALSE` (il en était déjà membre) sont tous deux
/// acceptables ; seul `RPC_E_CHANGED_MODE` — ce fil appartient déjà à un
/// appartement à thread unique — doit faire échouer l'ouverture.
///
/// ⚠️ **Pas de `CoUninitialize` en regard**, comme dans `wasapi.rs` : le fil de
/// rendu vit aussi longtemps que le processus, et il n'est jamais recyclé.
pub fn rejoindre_mta() -> Result<()> {
    // SAFETY : appel COM sans pointeur, sur le fil courant.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr == RPC_E_CHANGED_MODE {
        bail!(
            "ecriture du micro refusee : le fil de rendu appartient deja a un appartement a \
             thread unique (STA), pas a l'appartement multi-thread (MTA) qu'exige WASAPI"
        );
    }
    Ok(())
}

/// Le flux de rendu ouvert, prêt à recevoir des échantillons.
pub struct RenduWasapi {
    client: IAudioClient,
    rendu: IAudioRenderClient,
    /// `None` en mode [`Reveil::Echeance`].
    evenement: Option<HANDLE>,
    /// `GetBufferSize()`, en trames par canal.
    taille_tampon: u32,
    canaux: usize,
    description: String,
    reveil: Reveil,
}

impl RenduWasapi {
    /// Ouvre le flux de rendu sur `peripherique`, en mode **PARTAGÉ**, et le
    /// démarre.
    ///
    /// Le mode exclusif a été pesé et écarté (Décision 7 du plan E2) : rien
    /// n'établit qu'un flux exclusif traverse le câble — on y court-circuite
    /// le moteur audio de Windows, et que le pilote VB-Cable relaie encore
    /// vers CABLE Output est **inconnu** —, et le seul format mesuré est celui
    /// du mode partagé, `GetMixFormat` ne décrivant que lui.
    ///
    /// **Refuse** tout format qui n'est pas 48 kHz stéréo flottant 32 bits, en
    /// nommant ce qui a été rencontré : la règle est pure et vit dans
    /// `wasapi/format.rs`, où elle est éprouvée sur l'hôte.
    ///
    /// ⚠️ **Précondition : le fil courant est membre de la MTA** — appeler
    /// [`rejoindre_mta`] d'abord. Cette fonction n'exporte aucun objet COM
    /// vers un autre fil, elle suppose donc être appelée sur celui qui s'en
    /// servira.
    pub fn ouvrir(peripherique: &IMMDevice) -> Result<Self> {
        // SAFETY : `peripherique` vient d'un énumérateur créé sur ce fil (voir
        // la précondition), et chaque appel ci-dessous est le suivant immédiat
        // d'un appel réussi.
        unsafe {
            let client: IAudioClient = peripherique
                .Activate(CLSCTX_ALL, None)
                .context("activation du client audio de rendu (cable)")?;

            let mix = FormatMixage(
                client
                    .GetMixFormat()
                    .context("lecture du format de mixage du cable")?,
            );
            let canaux = mix.nChannels as usize;
            let frequence = mix.nSamplesPerSec;
            let bits = mix.wBitsPerSample;

            let flottant = if mix.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
                // `WAVEFORMATEXTENSIBLE` est `repr(packed)` : prendre une
                // référence sur `SubFormat` — ce que fait `==` sur un GUID —
                // est un accès non aligné, donc un comportement indéfini.
                let ext = mix.0 as *const WAVEFORMATEXTENSIBLE;
                let sous_format = std::ptr::addr_of!((*ext).SubFormat).read_unaligned();
                sous_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                mix.wFormatTag == WAVE_FORMAT_IEEE_FLOAT
            };

            wasapi_format::verifier(frequence, canaux, bits, flottant)?;
            let description = wasapi_format::decrire(frequence, canaux, bits, flottant);

            // Décision 8 : on TENTE l'événement, on ne le suppose pas.
            //
            // ⚠️ **Un `IAudioClient` ne s'initialise qu'une fois.** Un
            // `Initialize` refusé le laisse dans un état où un second appel
            // rendrait `AUDCLNT_E_ALREADY_INITIALIZED` ou pire : le repli
            // ré-ACTIVE donc un client neuf plutôt que de réessayer sur
            // celui-ci. C'est le même geste que `set_encode_size` depuis D5 —
            // détruire avant de reconstruire —, et pour la même raison.
            let (client, reveil) = match client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                DUREE_TAMPON_100NS,
                0,
                mix.0,
                None,
            ) {
                Ok(()) => (client, Reveil::Evenement),
                Err(e) => {
                    tracing::warn!(
                        erreur = %e,
                        "AUDCLNT_STREAMFLAGS_EVENTCALLBACK refuse par le cable : repli sur une \
                         boucle a echeance (spec §6 le predisait supporte, ce n'etait qu'une \
                         prediction)"
                    );
                    let neuf: IAudioClient = peripherique
                        .Activate(CLSCTX_ALL, None)
                        .context("re-activation du client audio de rendu apres refus de \
                                  l'evenement")?;
                    neuf.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        0,
                        DUREE_TAMPON_100NS,
                        0,
                        mix.0,
                        None,
                    )
                    .context("initialisation du client audio de rendu (cable, sans evenement)")?;
                    (neuf, Reveil::Echeance)
                }
            };
            // `mix` sort de portée en fin de bloc `unsafe` et libère alors le
            // format par `CoTaskMemFree` — APRÈS `Initialize`, qui en a fait sa
            // copie interne, comme l'exige la documentation de `GetMixFormat`.

            let evenement = if reveil == Reveil::Evenement {
                let handle = CreateEventW(None, false, false, PCWSTR::null())
                    .context("creation de l'evenement de reveil du rendu")?;
                client
                    .SetEventHandle(handle)
                    .context("liaison de l'evenement au client de rendu")?;
                Some(handle)
            } else {
                None
            };

            let taille_tampon = client
                .GetBufferSize()
                .context("lecture de la taille du tampon de rendu")?;
            let rendu: IAudioRenderClient = client
                .GetService()
                .context("obtention du service de rendu")?;
            client.Start().context("demarrage du rendu")?;

            Ok(Self {
                client,
                rendu,
                evenement,
                taille_tampon,
                canaux,
                description,
                reveil,
            })
        }
    }

    /// Format réellement obtenu, pour le journal.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Comment le fil est réveillé — **MESURÉ**, jamais supposé.
    pub fn reveil(&self) -> Reveil {
        self.reveil
    }

    /// Attend qu'il y ait de la place, au plus `delai`. Rend le nombre de
    /// **trames par canal** à fournir, ou `0`.
    ///
    /// `0` n'est pas une erreur : c'est un tour où le tampon était encore
    /// plein. L'appelant le compte comme un retard d'échéance — c'est ce
    /// compteur qui rendra décidable la question, aujourd'hui conjecturale, de
    /// savoir si le `Mutex` du lecteur mérite d'être remplacé par une file
    /// sans verrou.
    pub fn attendre_place(&mut self, delai: Duration) -> Result<usize> {
        let echeance = Instant::now() + delai;
        loop {
            let place = self.place()?;
            if place > 0 {
                return Ok(place);
            }
            let reste = echeance.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return Ok(0);
            }
            match self.reveil {
                Reveil::Evenement => {
                    let handle = self
                        .evenement
                        .context("mode evenement sans handle : incoherence interne")?;
                    // `as u32` après `min` : une attente de plus de 49 jours
                    // n'a aucun sens ici, et `INFINITE` (0xFFFFFFFF) ne doit
                    // jamais être atteint par accident — un fil de rendu qui
                    // attend sans borne ne se réveillerait plus si le
                    // périphérique disparaissait.
                    let ms = reste.as_millis().min(u32::MAX as u128 - 1) as u32;
                    // SAFETY : `handle` vient d'un `CreateEventW` réussi et
                    // n'est fermé que par `Drop`.
                    let issue = unsafe { WaitForSingleObject(handle, ms) };
                    if issue == WAIT_FAILED {
                        bail!("attente de l'evenement de rendu echouee (WAIT_FAILED)");
                    }
                }
                Reveil::Echeance => {
                    // Sondage borné : jamais plus que ce qui reste, et jamais
                    // plus qu'un quart de la période usuelle du moteur audio —
                    // sans quoi on dormirait au-delà du réveil suivant.
                    std::thread::sleep(reste.min(Duration::from_millis(2)));
                }
            }
        }
    }

    /// La place libre dans le tampon, en trames par canal.
    fn place(&self) -> Result<usize> {
        // SAFETY : `client` vient d'un `Initialize` réussi.
        let occupe = unsafe { self.client.GetCurrentPadding() }
            .context("lecture de l'occupation du tampon de rendu")?;
        Ok(self.taille_tampon.saturating_sub(occupe) as usize)
    }

    /// Écrit `trames` par canal depuis `pcm` (stéréo entrelacé, `f32`).
    ///
    /// `pcm` doit porter au moins `trames * canaux` échantillons ; le format
    /// ayant été refusé s'il n'était pas stéréo flottant 32 bits
    /// (`wasapi/format.rs`), la copie est un `memcpy` et rien d'autre.
    pub fn ecrire(&mut self, pcm: &[f32], trames: usize) -> Result<()> {
        if trames == 0 {
            return Ok(());
        }
        let besoin = trames * self.canaux;
        ensure!(
            pcm.len() >= besoin,
            "tampon PCM trop court : {} echantillons pour {trames} trames x {} canaux",
            pcm.len(),
            self.canaux
        );

        // ⚠️ **L'injection est testée AVANT `GetBuffer`, jamais entre lui et
        // `ReleaseBuffer`.** Un tampon acquis et jamais relâché bloquerait le
        // rendu pour de bon, et l'on mesurerait alors l'instrument.
        if faute_a_injecter() {
            bail!("faute injectee (MICRO_FAUTE_ECRITURE)");
        }

        // SAFETY : `GetBuffer(trames)` n'est appelé qu'après avoir vérifié par
        // `attendre_place` qu'autant de trames sont libres ; il rend un
        // pointeur sur `trames * canaux` échantillons du format négocié, donc
        // des `f32`, et `ReleaseBuffer` le rend à WASAPI dans le même bloc.
        unsafe {
            let tampon = self
                .rendu
                .GetBuffer(trames as u32)
                .context("acquisition du tampon de rendu")?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), tampon as *mut f32, besoin);
            self.rendu
                .ReleaseBuffer(trames as u32, 0)
                .context("liberation du tampon de rendu")?;
        }
        Ok(())
    }
}

impl Drop for RenduWasapi {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
            if let Some(handle) = self.evenement.take() {
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Le budget d'injection de fautes d'écriture, **GLOBAL AU PROCESSUS**.
///
/// 🔴 **Jamais relu par fil, et c'est la leçon que D10 a payée d'une passe
/// entière** (`AUDIO_FAUTE_LECTURE`, tâche 14) : un budget relu par fil se
/// réarme intégralement à chaque reconstruction — `std::env::var` rendant la
/// même valeur au fil neuf —, donc chaque fil meurt à son tour avant tout
/// appel réel, et **le chiffre-juge ne peut pas quitter zéro sur un produit
/// pourtant corrigé**. Un `static` décrémenté par `fetch_update` fait que le
/// budget s'épuise **une fois** pour tout le processus.
///
/// ⚠️ **Variable de BANC, jamais une configuration livrée.** Absente ou nulle :
/// aucune faute, et pas une ligne de journal.
fn faute_a_injecter() -> bool {
    use std::sync::atomic::{AtomicU32, Ordering};
    static BUDGET: std::sync::OnceLock<AtomicU32> = std::sync::OnceLock::new();
    let budget = BUDGET.get_or_init(|| {
        let n: u32 = std::env::var("MICRO_FAUTE_ECRITURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            // Un SEUL `warn!` : deux traces au même instant se compteraient
            // comme deux événements (piège maison du sous-bloc D6).
            tracing::warn!(
                fautes_a_injecter = n,
                "injection de fautes d'ecriture du micro ARMEE (banc, MICRO_FAUTE_ECRITURE)"
            );
        }
        AtomicU32::new(n)
    });
    budget
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
        .is_ok()
}
