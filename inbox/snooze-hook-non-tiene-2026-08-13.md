
## Update: 0.4.2 -> 0.4.5 NON risolve
Testato su myc 0.4.5 (dopo un tentativo di fix): `myc followup snooze` conferma
ancora "snoozed for the next 5 stop(s)", ma allo stop SUCCESSIVO l'hook ri-spara
comunque il MYCELIUM FOLLOW-UP CHECK con gli stessi 3 follow-up aperti.
Quindi il fix nelle 0.4.x non ha colpito la causa. Rafforza l'ipotesi 3
(cwd/due-DB): il comando snooze e l'hook potrebbero operare su DB/scope diversi,
oppure l'hook non consulta affatto lo stato di snooze prima di bloccare.
Da verificare NEL codice dell'hook myc-followup-stop.sh: legge il contatore di
snooze? da quale path? e' lo stesso che `snooze` scrive?

## Update: 0.4.7 sembra risolvere (ipotesi 3 confermata)
myc 0.4.7: l'hook .claude/hooks/myc-followup-stop.sh ora (righe 43-60) CAMMINA SU
dagli antenati fino al primo .mycelium/mycelium.db e legge .followup-snooze da LI',
per matchare get_db_path() di myc. Il commento nell'hook cita esplicitamente
"the snooze doesn't stick bug". CAUSA CONFERMATA = ipotesi 3: c'erano DUE
.mycelium/mycelium.db (root poppix + beebeeboard-workspace); snooze e hook potevano
risolvere progetti diversi -> lo snooze finiva in un .mycelium che l'hook non
leggeva. Verificato: snooze da root scrive ./.mycelium/.followup-snooze (contatore 5)
e l'hook cammina su fino allo stesso ./.mycelium. Da confermare allo stop che NON
ri-nagga. Nota correlata: l'errore "No such file: .claude/hooks/myc-followup-stop.sh"
(path relativo) succede quando la cwd e' una sottocartella senza .claude/hooks -
problema distinto dal snooze, ma stessa radice (dipendenza dalla cwd).

## Bug DISTINTO (stesso file): path relativo dell'hook si rompe in sottocartella
Sintomo: `Stop hook error: /bin/sh: .claude/hooks/myc-followup-stop.sh: No such
file or directory`, comparso "dall'ultima versione di mycelium".
Causa: `myc hooks install` scrive nel <project>/.claude/settings.json un command
con path RELATIVO ".claude/hooks/myc-followup-stop.sh". /bin/sh lo risolve dalla
CWD corrente. Se si lavora in una SOTTOCARTELLA del progetto (es. poppix/api_beebeeboard/),
la cwd non e' la root -> l'hook non e' in ./.claude/hooks -> "No such file".
Fix applicato a mano nel settings.json del progetto: path assoluto via
"$CLAUDE_PROJECT_DIR/.claude/hooks/myc-followup-stop.sh" (Claude Code espande
CLAUDE_PROJECT_DIR alla root del progetto -> funziona da qualsiasi cwd).
RACCOMANDAZIONE per myc hooks install: scrivere il path con $CLAUDE_PROJECT_DIR
(o assoluto), non relativo, altrimenti si rompe per chiunque lavori in una subdir.
Nota: la registrazione GLOBALE (~/.claude/settings.json) usava gia' $HOME/... assoluto
e funzionava; solo quella project-local era relativa.

## AGGIORNAMENTO 2026-08-13 (diagnosi finale, corregge quanto sopra)

Due bug DISTINTI, ora entrambi chiari:

1. **Hook "No such file" in subdir** — FIXATO. `poppix/.claude/settings.json:43`
   registrava l'hook con path RELATIVO `.claude/hooks/myc-followup-stop.sh`.
   /bin/sh lo risolve dalla cwd; con cwd = subdir (`api_beebeeboard/`) non lo
   trova. Fix: `$CLAUDE_PROJECT_DIR/.claude/hooks/myc-followup-stop.sh`.

2. **Snooze "non teneva"** — NON era un bug di myc. Era mio errore d'uso +
   due-DB. Fatti reali (myc 0.4.7):
   - `myc followup snooze` NON prende un ID. E' GLOBALE: silenzia l'hook per
     le prossime N stop (`-t N`, default 5), project-scoped. Il mio ricordo
     "snooze per-followup" era falso.
   - Scrive `.followup-snooze` nel primo `.mycelium` salendo dalla cwd.
   - Esistono DUE `.mycelium/mycelium.db`: `poppix/` e
     `poppix/beebeeboard-workspace/`. Se snoozi da `beebeeboard-workspace/`
     scrivi il file LI'; ma l'hook, girando da `api_beebeeboard/`, risolve
     `poppix/.mycelium` -> non vede lo snooze -> continua a naggare.
   - Fix operativo: snoozare dalla STESSA cwd da cui gira l'hook (una
     sottodir di poppix che NON contiene un suo `.mycelium`, cosi' entrambi
     risolvono `poppix/.mycelium`). Fatto: `poppix/.mycelium/.followup-snooze`
     = 20.

   Miglioria possibile per myc: `snooze` e l'hook dovrebbero ancorare al
   `.mycelium` del PROJECT_DIR (root del repo git), non al primo salendo dalla
   cwd, cosi' due sottodir con `.mycelium` propri non divergono.
