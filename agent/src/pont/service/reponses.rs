//! **What we do with a browser response**: pairing what we
//! expected with what arrived, and the write that follows from it.
//!
//! # Why this extraction, and WHY IT COMES ONE ADDITION TOO LATE
//!
//! `service.rs` stood at **436** lines when F3 opened it — and not the 325 that
//! §2.2 of the plan announced, F2 having made it grow in the meantime. The
//! census of codes of task 5 took it to **510**: **the ceiling of
//! 500 was CROSSED**, and it is caught up by this extraction — **never by
//! a compression**, which `CLAUDE.md` forbids by name and which D9 paid for twice
//! before having to extract anyway.
//!
//! ⚠️ **The right gesture would have been to extract BEFORE adding**, as D9
//! invented it (`capteur/serveur/instances.rs`) and as D10 played it three times.
//! That is not what happened: the plan budgeted a margin of 175 that
//! no longer existed, and the crossing was noticed by the command after
//! the addition. **Declared rather than hidden**, and tasks 8 and 13, which
//! still add to this file, now have the margin.
//!
//! # The dividing line
//!
//! [`super`] carries **the loop** — receive, sweep, expire, take census,
//! complete. This module carries **the pairing**: `(what we expected, the
//! retained ProjFS context)` against `(the received type, the header, the payload)`. The
//! two responsibilities are reviewed separately, and it is the only reason worth
//! splitting a file.
//!
//! ⚠️ **NO BODY LINE IS MODIFIED** by the extraction itself: the
//! transposition is VERBATIM, and the check is a text-to-text
//! comparison recorded in task 5's log. What task 13 will add to it comes
//! in a separate commit, so that the review can compare one with the other.

use std::sync::atomic::Ordering;

use windows::core::HRESULT;
use windows::Win32::Foundation::S_OK;

use crate::pont::ecriture::fil::Ordre;
use crate::pont::enumeration::Session;
use crate::pont::errors::Error;
use crate::pont::projfs::{ContexteProjFs, Etat};
use crate::pont::table::Attendue;
use proto::files::entetes;

use super::verbes;

/// What remains to do after applying a response.
pub(super) enum Suite {
    Termine(HRESULT),
    Poursuit,
}

/// Completes, taking into account that an enumeration requires extended
/// parameters.
pub(super) fn terminer(
    etat: &Etat,
    commande: Option<i32>,
    contexte: Option<ContexteProjFs>,
    result: HRESULT,
) {
    match contexte {
        Some(ContexteProjFs::Enumeration { tampon, .. }) => match commande {
            Some(commande) => verbes::completer_enumeration(etat, commande, tampon.0, result),
            // An enumeration context without a command does not exist; saying so
            // rather than ignoring it.
            None => tracing::warn!("contexte d'énumération sans commande ProjFS : ignoré"),
        },
        _ => verbes::completer(etat, commande, result),
    }
}

pub(super) fn appliquer(
    etat: &Etat,
    correlation: u32,
    commande: Option<i32>,
    attendue: Attendue,
    trame: &proto::files::Trame<'_>,
    contexte: Option<&ContexteProjFs>,
) -> Suite {
    match (attendue, contexte) {
        (Attendue::Attributs { chemin }, Some(ContexteProjFs::Attributs { chemin_projfs })) => {
            let Ok(meta) = serde_json::from_slice::<entetes::Meta>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Meta illisible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            };
            // 🔴 **THE PLACEHOLDER IS CREATED UNDER THE **STORED** NAME, never under
            // the one the application typed** — it is consequence ① of
            // F3's case canonicaliser.
            //
            // ⚠️ **We only reconvert the path IF there is something to
            // change.** F1 gave itself the property of never touching the
            // bytes ProjFS delivered — "a round trip where a case or a
            // separator could be lost" —, and
            // `chemins::with_last_component` returns `None` when the canonical
            // name is already the path's.
            let projfs_texte = String::from_utf16_lossy(
                chemin_projfs.strip_suffix(&[0u16]).unwrap_or(chemin_projfs),
            );
            let canonique = crate::pont::chemins::with_last_component(&projfs_texte, &meta.nom);
            let Some(neuf) = canonique else {
                return Suite::Termine(verbes::write_placeholder(
                    etat,
                    chemin_projfs,
                    meta.repertoire,
                    meta.size,
                    meta.modified,
                ));
            };
            tracing::debug!(
                demande = %projfs_texte,
                stocke = %neuf,
                "nom canonique : le substitut prend le nom du poste local"
            );
            let neuf_utf16: Vec<u16> = neuf.encode_utf16().chain(std::iter::once(0)).collect();
            let issue = verbes::write_placeholder(
                etat,
                &neuf_utf16,
                meta.repertoire,
                meta.size,
                meta.modified,
            );
            if issue.is_ok() {
                return Suite::Termine(issue);
            }
            // 🔴 **THE FALLBACK, AND IT IS DECLARED.** That ProjFS accepts a
            // placeholder whose name differs from the one requested is the
            // documented way to correct a case — **but it has never been
            // MEASURED on this VM**, and a refusal would make the file
            // unopenable whereas it opens today. We therefore retry under
            // the requested name rather than fail, and **the log says
            // which of the two paths served**.
            tracing::warn!(
                demande = %projfs_texte,
                stocke = %neuf,
                %issue,
                "PrjWritePlaceholderInfo a refuse le nom canonique : repli sur le nom demande"
            );
            Suite::Termine(verbes::write_placeholder(
                etat,
                chemin_projfs,
                meta.repertoire,
                meta.size,
                meta.modified,
            ))
        }
        // `QueryFileName`: the name exists, and that is ALL ProjFS expects.
        // Writing a marker here would create a projected object for a file
        // no one opens. A `TYPE_ECHEC` is handled upstream and returns
        // `ERROR_FILE_NOT_FOUND`, which feeds the negative cache.
        (Attendue::Attributs { .. }, Some(ContexteProjFs::Existence)) => Suite::Termine(S_OK),
        (
            Attendue::Lire {
                chemin,
                position,
                length,
            },
            Some(ContexteProjFs::Lecture { flux, fenetre }),
        ) => {
            let Ok(entete) = serde_json::from_slice::<entetes::Data>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Donnees illisible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            };
            // ⚠️ **The header and the payload must corroborate each other.** Writing into
            // ProjFS's buffer a quantity of bytes the sender did not
            // believe it sent is the kind of divergence no downstream check
            // catches: the file would be truncated or lengthened, and
            // only a digest would say so.
            if entete.length as usize != trame.charge.len()
                || entete.position != position
                || entete.length != length
            {
                tracing::warn!(
                    chemin,
                    position,
                    length,
                    recu_position = entete.position,
                    received_length = entete.length,
                    octets = trame.charge.len(),
                    "réponse Donnees incohérente avec la plage demandée : jetée"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            }
            // 🔴 **THE ORDER IS CHECKED BEFORE WRITING, NEVER AFTER.** An
            // out-of-order response written then denounced would already have corrupted the
            // file, and **only a SHA-256 digest would say so**.
            let mut garde = match fenetre.lock() {
                Ok(g) => g,
                Err(empoisonne) => empoisonne.into_inner(),
            };
            if let Err(hors) = garde.recu(position) {
                tracing::warn!(
                    chemin,
                    recue = hors.recue,
                    attendue = hors.attendue,
                    "reponse de lecture HORS D'ORDRE : jetee, RIEN n'est ecrit"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            }
            let issue = verbes::write_file_data(etat, flux.0, position, trame.charge);
            if issue.is_err() {
                return Suite::Termine(issue);
            }
            etat.octets_hydrates
                .fetch_add(trame.charge.len() as u64, Ordering::Relaxed);

            // ✅ **F3'S WINDOW REPLACES "ONE CHUNK IN FLIGHT AT A
            // TIME".** *(These lines said: "ONE chunk in flight at a time
            // […] flow control through `bufferedAmount` and `SEUIL_TAMPON`
            // is a deliverable of F3, not F1. Implementing it halfway here
            // would be worse than not implementing it." F3 has arrived, and it
            // did NOT implement half of it: the bridge's window
            // (`pont::lecture`) AND the browser's back-pressure
            // (`client/src/fichiers/flux.ts`) are delivered together — with a
            // single chunk in flight, the rule of spec §7.3 could NEVER
            // bite.)*
            //
            // 🔴 **THE ORDERING INVARIANT IS CHECKED, NEVER BELIEVED.** The channel is
            // `ordered` and chunks are requested in increasing
            // positions; `Fenetre::recu` nevertheless **denounces** an out-of-order
            // response instead of applying it. Writing a range at the wrong
            // rank would produce a file that **only a SHA-256 digest**
            // would show to be wrong — the one F1 NEVER established.
            let lot = garde.a_demander();
            let terminee = garde.terminee();
            let en_vol_max = garde.en_vol_max();
            drop(garde);
            if terminee {
                etat.entrees_hydratees.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(chemin, correlation, en_vol_max, "lecture complète");
                return Suite::Termine(S_OK);
            }
            // A read ALWAYS carries a ProjFS command: it is a `GetFileData`
            // callback that registered it.
            let Some(commande) = commande else {
                tracing::warn!(chemin, "lecture sans commande ProjFS : impossible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            };
            for morceau in lot {
                etat.demander_lecture(
                    commande,
                    &chemin,
                    morceau,
                    flux.0,
                    std::sync::Arc::clone(fenetre),
                );
            }
            // ⚠️ **`Poursuit` EVEN WHEN THE BATCH IS EMPTY**: other
            // correlations of the SAME command are still in flight, and
            // completing here would let them answer a finished callback.
            Suite::Poursuit
        }
        (
            Attendue::Lister {
                chemin,
                enumeration,
            },
            Some(ContexteProjFs::Enumeration {
                tampon, expression, ..
            }),
        ) => {
            let Ok(entete) = serde_json::from_slice::<entetes::Entrees>(trame.entete) else {
                tracing::warn!(chemin, "en-tête Entrees illisible");
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            };
            // **F5** — the cache memorises the RAW entries, before `preparer`.
            // See `pont::cache`: the prepared result depends on the request, and
            // memorising it would make a `dir *.txt` poison the next
            // `dir`.
            let brutes = verbes::entrees_depuis(entete.entrees);
            if etat.cache_arme {
                if let Ok(mut cache) = etat.cache.lock() {
                    cache.poser(chemin.clone(), brutes.clone(), std::time::Instant::now());
                }
            }
            let entrees = crate::pont::enumeration::preparer(
                brutes,
                expression.as_deref(),
                |nom, motif| etat.apparier(nom, motif),
                |a, b| etat.comparer(a, b),
            );
            let mut sessions = match etat.sessions.lock() {
                Ok(sessions) => sessions,
                Err(_) => return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue))),
            };
            let session = sessions.entry(enumeration).or_insert_with(Session::new);
            session.poser(entrees);
            Suite::Termine(verbes::remplir(etat, session, tampon.0))
        }
        // 🔴 **THE TWO WRITE ARMS.** They complete NO callback —
        // `command_id` is `None` — and only relay the acknowledgement to the
        // write thread, which decides whether it pushes the next chunk or removes
        // the entry from the journal.
        //
        // ⚠️ **The ProjFS context is `None` here, and it is not an anomaly**
        // : a write has neither an enumeration buffer nor a data stream.
        (Attendue::Write { chemin, last }, None) => {
            if trame.type_message != proto::files::TYPE_FAIT {
                tracing::warn!(
                    chemin,
                    correlation,
                    type_message = trame.type_message,
                    "réponse d'un type inattendu à une écriture : jetée"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            }
            tracing::debug!(chemin, correlation, last, "morceau d'écriture acquitté");
            let _ = etat.vers_ecriture.send(Ordre::Fait { correlation });
            Suite::Termine(S_OK)
        }
        // 🔴 **THE MUTATION (F3).** It completes NO callback — `command_id`
        // is `None` — and only relays the acknowledgement to the thread, which decides
        // whether it pushes the next one.
        //
        // ⚠️ **The ProjFS context is `None` here, and it is not an anomaly**
        // : a mutation has neither an enumeration buffer nor a data stream. It
        // arises from a POST notification, which has already returned control to
        // the application.
        (
            Attendue::Muter {
                chemin,
                renommage,
                destination,
            },
            None,
        ) => {
            if trame.type_message != proto::files::TYPE_FAIT {
                tracing::warn!(
                    chemin,
                    correlation,
                    renommage,
                    type_message = trame.type_message,
                    "reponse d'un type inattendu a une mutation : jetee"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            }
            tracing::debug!(chemin, correlation, renommage, "mutation acquittee");
            // **F5 — SECOND HALF OF INVALIDATION: what the BROWSER
            // did.** It does not duplicate the first, it closes the window
            // the first leaves: between the notification and this acknowledgement,
            // the local workstation had not changed yet, and a listing could have
            // memorised there — legitimately — a content that becomes wrong HERE.
            invalider_le_cache(etat, &chemin, destination.as_deref());
            let _ = etat.vers_ecriture.send(Ordre::Fait { correlation });
            Suite::Termine(S_OK)
        }
        (Attendue::Create { chemin }, None) => {
            if trame.type_message != proto::files::TYPE_FAIT {
                tracing::warn!(
                    chemin,
                    correlation,
                    type_message = trame.type_message,
                    "réponse d'un type inattendu à une création : jetée"
                );
                return Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)));
            }
            tracing::debug!(chemin, correlation, "création acquittée");
            invalider_le_cache(etat, &chemin, None);
            let _ = etat.vers_ecriture.send(Ordre::Fait { correlation });
            Suite::Termine(S_OK)
        }
        // A response whose type does not match what the command
        // expected. It is not applied "as best we can": the browser and
        // the bridge diverge, and guessing would write anything into
        // ProjFS's buffer.
        (attendue, contexte) => {
            tracing::warn!(
                correlation,
                type_message = trame.type_message,
                ?attendue,
                contexte_present = contexte.is_some(),
                "réponse d'un type qui ne correspond pas à la commande : jetée"
            );
            Suite::Termine(HRESULT(etat.compteurs.rendre(Error::Inattendue)))
        }
    }
}

/// Forgets what the directories an acknowledged mutation changed
/// contained — **the source's parent, and the destination's if there is
/// one**.
///
/// ⚠️ **Inert if `PONT_CACHE=0`**, like everything F5 adds: the disarmed
/// arm must behave EXACTLY like the product before F5, otherwise
/// criterion ①'s red would measure something other than the absence of a cache.
fn invalider_le_cache(etat: &Etat, source: &str, destination: Option<&str>) {
    if !etat.cache_arme {
        return;
    }
    if let Ok(mut cache) = etat.cache.lock() {
        cache.invalider(source);
        if let Some(vers) = destination {
            cache.invalider(vers);
        }
    }
}
