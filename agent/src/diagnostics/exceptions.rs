//! Filtre d'exception : consigne la pile d'appel AU MOMENT de la faute.
//!
//! Chantier `2026-07-31-duplications-paralleles`, tâche 2bis. Un plantage
//! intermittent (`0xc0000005` dans `ntdll.dll`, décalage `0x19daa`) emporte le
//! processus à la destruction d'un `H264Encoder`. Deux rondes ont cherché à
//! **deviner** l'appel fautif en encadrant des appels de traces ; ce module le
//! **lit**.
//!
//! # Trois choix qui ne sont pas cosmétiques
//!
//! 1. **`AddVectoredExceptionHandler` en PREMIÈRE chance** (et non
//!    `SetUnhandledExceptionFilter` seul) : il voit l'exception avant tout
//!    gestionnaire, donc y compris si quelque chose l'avale. Le filtre final
//!    est posé en plus, pour distinguer la faute qui tue le processus de
//!    celles qui sont rattrapées.
//! 2. **Aucune allocation dans le gestionnaire, et pas de `tracing`.** La
//!    faute relevée est dans `RtlpEnterCriticalSectionContended` : si la
//!    section critique en cause est celle d'un tas, allouer depuis le
//!    gestionnaire replanterait ou bloquerait. La table des modules est
//!    photographiée à l'installation, le message est formaté dans un tampon
//!    de pile et écrit par un seul `write`.
//! 3. **Journal séparé de `agent.log`.** La sortie standard de l'agent
//!    traverse un tuyau PowerShell (`scripts/run-agent.sh`) : ce qui y est en
//!    vol quand le processus meurt est perdu. Ce fichier-ci est écrit
//!    directement, puis synchronisé.
//!
//! Le gestionnaire ne s'exécute qu'au plantage : contrairement aux traces
//! posées dans `Drop` par la ronde 2, il ne change pas le minutage avant la
//! faute, donc il ne devrait pas faire fuir un défaut sensible au minutage.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, EXCEPTION_ACCESS_VIOLATION};
use windows::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, RtlCaptureStackBackTrace, SetUnhandledExceptionFilter, CONTEXT,
    EXCEPTION_POINTERS, EXCEPTION_RECORD,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
};
use windows::Win32::System::Threading::GetCurrentThreadId;

/// Un module chargé, tel que photographié à l'installation.
struct Module {
    base: usize,
    taille: usize,
    nom: String,
}

/// Photographie de la table des modules. Lue — jamais écrite — depuis le
/// gestionnaire, donc sans allocation au moment de la faute.
static MODULES: OnceLock<Vec<Module>> = OnceLock::new();
/// Journal dédié, ouvert une fois à l'installation.
static JOURNAL: OnceLock<File> = OnceLock::new();
/// Identifiant du fil qui a installé le filtre — celui de `main`. Sert à dire
/// si la faute survient sur CE fil ou sur un fil de travail d'un tiers
/// (Media Foundation, pilote NVIDIA), ce que rien n'établissait jusqu'ici.
static FIL_PRINCIPAL: AtomicU32 = AtomicU32::new(0);
/// Plafond de rapports : une violation d'accès en boucle ne doit pas noyer le
/// journal ni ralentir la mort.
static RAPPORTS: AtomicU32 = AtomicU32::new(0);
/// Garde de non-réentrance : si le gestionnaire lui-même faute, ne pas
/// repartir dedans.
static DANS_LE_GESTIONNAIRE: AtomicBool = AtomicBool::new(false);

const PLAFOND_RAPPORTS: u32 = 8;
const CONTINUER_LA_RECHERCHE: i32 = 0;

/// Installe le filtre si `AGENT_TRACE_EXCEPTIONS` est posée à autre chose que
/// `0`. Silencieux et sans effet sinon.
///
/// Le chemin du journal se règle par `AGENT_TRACE_EXCEPTIONS_FICHIER` ; il
/// vaut `C:\dev\exceptions.log` par défaut, à côté de `agent.log`.
pub(crate) fn installer() {
    if std::env::var("AGENT_TRACE_EXCEPTIONS").is_ok_and(|v| v != "0") {
        // Ne rien faire de plus si l'ouverture du journal échoue : un
        // diagnostic ne doit jamais empêcher la mesure de tourner.
        let chemin = std::env::var("AGENT_TRACE_EXCEPTIONS_FICHIER")
            .unwrap_or_else(|_| r"C:\dev\exceptions.log".to_string());
        match File::create(&chemin) {
            Ok(fichier) => {
                let _ = JOURNAL.set(fichier);
            }
            Err(e) => {
                tracing::warn!(chemin, erreur = %e, "journal d'exceptions non ouvert");
                return;
            }
        }
        let _ = MODULES.set(photographier_les_modules());
        FIL_PRINCIPAL.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);

        unsafe {
            // `1` = en tête de chaîne, donc avant tout gestionnaire déjà posé.
            AddVectoredExceptionHandler(1, Some(filtre_vectorise));
            SetUnhandledExceptionFilter(Some(filtre_final));
        }
        let nombre = MODULES.get().map(|m| m.len()).unwrap_or(0);
        tracing::info!(
            chemin,
            modules = nombre,
            fil_principal = FIL_PRINCIPAL.load(Ordering::SeqCst),
            "filtre d'exception installé"
        );

        if std::env::var("AGENT_TRACE_EXCEPTIONS_AUTOTEST").is_ok_and(|v| v != "0") {
            autotest();
        }
    }
}

/// Provoque délibérément une violation d'accès, pour **éprouver l'instrument
/// avant de se fier à son silence**.
///
/// Une campagne sans plantage ne prouve rien si l'on n'a pas montré que le
/// filtre aurait parlé. Ce chemin le montre : il tue le processus, produit un
/// rapport dans le journal dédié, et fait produire à WER son vidage — les
/// trois maillons de la chaîne de diagnostic, éprouvés d'un coup, sans
/// attendre le défaut intermittent.
///
/// Ne s'exécute que sur `AGENT_TRACE_EXCEPTIONS_AUTOTEST` posée à autre chose
/// que `0`, en plus de `AGENT_TRACE_EXCEPTIONS`.
fn autotest() {
    tracing::warn!("AUTOTEST du filtre d'exception : violation d'accès délibérée, le processus va mourir");
    // Écriture à une adresse non mappée volontairement basse et reconnaissable
    // dans le rapport (« adresse fautive »).
    let adresse = 0x24usize as *mut u32;
    unsafe { std::ptr::write_volatile(adresse, 0xdead_beef) };
}

/// Photographie base/taille/nom de chaque module chargé.
fn photographier_les_modules() -> Vec<Module> {
    let mut modules = Vec::new();
    unsafe {
        let Ok(instantane) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, 0) else {
            return modules;
        };
        let mut entree = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if Module32FirstW(instantane, &mut entree).is_ok() {
            loop {
                let fin = entree
                    .szModule
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entree.szModule.len());
                modules.push(Module {
                    base: entree.modBaseAddr as usize,
                    taille: entree.modBaseSize as usize,
                    nom: String::from_utf16_lossy(&entree.szModule[..fin]),
                });
                entree.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
                if Module32NextW(instantane, &mut entree).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(instantane);
    }
    modules
}

/// Tampon de formatage sur la pile : `write!` sans jamais toucher au tas.
struct Tampon {
    octets: [u8; 16384],
    ecrits: usize,
}

impl Tampon {
    fn neuf() -> Self {
        Self {
            octets: [0; 16384],
            ecrits: 0,
        }
    }
    fn contenu(&self) -> &[u8] {
        &self.octets[..self.ecrits]
    }
}

impl std::fmt::Write for Tampon {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let reste = self.octets.len() - self.ecrits;
        let n = s.len().min(reste);
        self.octets[self.ecrits..self.ecrits + n].copy_from_slice(&s.as_bytes()[..n]);
        self.ecrits += n;
        Ok(())
    }
}

/// Situe une adresse dans la table photographiée : `module+0xdécalage`.
fn situer(tampon: &mut Tampon, adresse: usize) {
    if let Some(modules) = MODULES.get() {
        for m in modules {
            if adresse >= m.base && adresse < m.base + m.taille {
                let _ = write!(tampon, "{}+0x{:x}", m.nom, adresse - m.base);
                return;
            }
        }
    }
    let _ = write!(tampon, "<hors module>");
}

/// Écrit un rapport complet dans le journal dédié. Aucune allocation.
unsafe fn consigner(etiquette: &str, enregistrement: *const EXCEPTION_RECORD, contexte: *const CONTEXT) {
    let Some(journal) = JOURNAL.get() else {
        return;
    };
    let mut t = Tampon::neuf();
    let code = unsafe { (*enregistrement).ExceptionCode.0 } as u32;
    let adresse = unsafe { (*enregistrement).ExceptionAddress } as usize;
    let fil = unsafe { GetCurrentThreadId() };

    let _ = writeln!(t, "=== exception ({etiquette}) ===");
    let _ = write!(t, "code=0x{code:08x} adresse=0x{adresse:016x} (");
    situer(&mut t, adresse);
    let _ = writeln!(t, ")");
    let _ = writeln!(
        t,
        "fil={} (fil principal={})",
        fil,
        FIL_PRINCIPAL.load(Ordering::Relaxed)
    );

    let nb_params = unsafe { (*enregistrement).NumberParameters } as usize;
    if code == EXCEPTION_ACCESS_VIOLATION.0 as u32 && nb_params >= 2 {
        let genre = unsafe { (*enregistrement).ExceptionInformation[0] };
        let fautive = unsafe { (*enregistrement).ExceptionInformation[1] };
        let libelle = match genre {
            0 => "lecture",
            1 => "écriture",
            8 => "exécution (DEP)",
            _ => "genre inconnu",
        };
        let _ = writeln!(
            t,
            "violation d'accès : {libelle} à l'adresse 0x{fautive:016x}"
        );
    }

    if !contexte.is_null() {
        let c = unsafe { &*contexte };
        let _ = writeln!(
            t,
            "registres : rip=0x{:016x} rsp=0x{:016x} rbp=0x{:016x}",
            c.Rip, c.Rsp, c.Rbp
        );
        let _ = writeln!(
            t,
            "            rax=0x{:016x} rcx=0x{:016x} rdx=0x{:016x} rbx=0x{:016x}",
            c.Rax, c.Rcx, c.Rdx, c.Rbx
        );
        let _ = writeln!(
            t,
            "            r8 =0x{:016x} r9 =0x{:016x} r12=0x{:016x} r13=0x{:016x}",
            c.R8, c.R9, c.R12, c.R13
        );
        let _ = writeln!(
            t,
            "            r14=0x{:016x} r15=0x{:016x} rsi=0x{:016x} rdi=0x{:016x}",
            c.R14, c.R15, c.Rsi, c.Rdi
        );
    }

    let mut cadres: [*mut core::ffi::c_void; 62] = [std::ptr::null_mut(); 62];
    let nombre = unsafe { RtlCaptureStackBackTrace(0, &mut cadres, None) } as usize;
    let _ = writeln!(t, "pile ({nombre} cadres, du plus récent au plus ancien) :");
    for (i, cadre) in cadres.iter().take(nombre).enumerate() {
        let a = *cadre as usize;
        let _ = write!(t, "  #{i:02} 0x{a:016x}  ");
        situer(&mut t, a);
        let _ = writeln!(t);
    }
    let _ = writeln!(t, "=== fin ===");

    // Un seul `write`, puis synchronisation : le processus peut mourir juste
    // après le retour de ce gestionnaire.
    let mut fichier = journal;
    let _ = fichier.write_all(t.contenu());
    let _ = fichier.flush();
    let _ = journal.sync_data();
}

/// Vrai si ce gestionnaire peut consigner (plafond non atteint, pas de
/// réentrance).
fn autorise() -> bool {
    if RAPPORTS.fetch_add(1, Ordering::SeqCst) >= PLAFOND_RAPPORTS {
        return false;
    }
    DANS_LE_GESTIONNAIRE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn relacher() {
    DANS_LE_GESTIONNAIRE.store(false, Ordering::SeqCst);
}

/// Gestionnaire vectorisé, première chance. Ne rattrape RIEN : il consigne et
/// rend la main à la chaîne normale (`EXCEPTION_CONTINUE_SEARCH`), pour que le
/// comportement du processus — y compris la production du vidage par WER —
/// reste exactement celui d'avant.
unsafe extern "system" fn filtre_vectorise(infos: *mut EXCEPTION_POINTERS) -> i32 {
    if infos.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    let enregistrement = unsafe { (*infos).ExceptionRecord };
    if enregistrement.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    // Seules les violations d'accès nous intéressent : une exception C++ ou
    // un point d'arrêt de première chance rempliraient le journal pour rien.
    if unsafe { (*enregistrement).ExceptionCode } != EXCEPTION_ACCESS_VIOLATION {
        return CONTINUER_LA_RECHERCHE;
    }
    if autorise() {
        unsafe { consigner("première chance", enregistrement, (*infos).ContextRecord) };
        relacher();
    }
    CONTINUER_LA_RECHERCHE
}

/// Filtre final : n'est appelé que si personne n'a traité l'exception, donc
/// uniquement pour celle qui tue le processus. Rend
/// `EXCEPTION_CONTINUE_SEARCH` pour laisser WER produire son vidage.
unsafe extern "system" fn filtre_final(infos: *const EXCEPTION_POINTERS) -> i32 {
    if infos.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    let enregistrement = unsafe { (*infos).ExceptionRecord };
    if enregistrement.is_null() {
        return CONTINUER_LA_RECHERCHE;
    }
    // Ici, pas de filtre sur le code : une exception non gérée est fatale
    // quelle qu'elle soit, et c'est exactement celle qu'on veut voir.
    RAPPORTS.store(0, Ordering::SeqCst);
    if autorise() {
        unsafe { consigner("NON GÉRÉE — fatale", enregistrement, (*infos).ContextRecord) };
        relacher();
    }
    CONTINUER_LA_RECHERCHE
}
