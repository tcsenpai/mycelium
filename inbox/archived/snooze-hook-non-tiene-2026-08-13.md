# Bug: `myc followup snooze` non silenzia lo Stop hook

Data: 2026-08-13
Segnalato da: sessione Claude Code su repo poppix

## Sintomo
Lo Stop hook `myc-followup-stop.sh` (installato da `myc init`) ri-spara il
MYCELIUM FOLLOW-UP CHECK a OGNI stop, anche subito dopo aver eseguito
`myc followup snooze`. Il comando conferma ogni volta:

    ✅ Follow-up check snoozed for the next 5 stop(s)

ma allo stop successivo l'hook blocca di nuovo con la stessa lista di follow-up
aperti. Osservato ripetutamente nella stessa sessione (5 follow-up aperti,
F136-F140): snooze eseguito ~4 volte, hook ri-partito ogni volta.

## Atteso (da AGENTS.md, sezione Follow-ups)
> Once you've surfaced the follow-ups to the user and they've chosen to leave
> them for later, run `myc followup snooze` to silence the hook for the next
> few stops instead of being re-prompted each turn. Snooze is project-scoped
> and consumes one stop at a time.

Quindi dopo `snooze` l'hook NON dovrebbe ri-bloccare per i successivi 5 stop.

## Ipotesi (da verificare nel codice myc / hook)
1. Lo snooze scrive un contatore/stato che l'hook NON legge (path diverso, o
   scope diverso: lo snooze e' "project-scoped" ma l'hook potrebbe guardare
   un altro progetto/cwd).
2. Il contatore "consuma uno stop alla volta" ma qualcosa lo azzera/reimposta
   a ogni run (es. l'hook o un altro comando myc lo resetta).
3. Race/cwd: lo snooze e' stato eseguito da una cwd diversa da quella in cui
   gira l'hook -> due DB mycelium diversi (trappola nota "myc due DB / cwd").
   Da controllare: `myc followup snooze` e l'hook devono operare sullo STESSO
   .mycelium/ (stesso progetto). Se lo snooze scrive nel DB di root poppix e
   l'hook legge quello di api_beebeeboard (o viceversa), non si vedono.

## Come riprodurre
1. Avere >=1 follow-up in stato `open`.
2. `myc followup snooze` -> conferma "snoozed for 5 stops".
3. Fermarsi (stop). L'hook ri-spara il check invece di restare silenzioso.

## Workaround attuale
Nessuno efficace via snooze. Per far tacere l'hook si deve togliere i
follow-up dallo stato `open` (chiuderli: done/wontfix, o promuoverli a task).

## Cosa verificare per il fix
- Dove `snooze` PERSISTE lo stato (file? colonna DB? quale .mycelium/?).
- Dove l'hook `myc-followup-stop.sh` LEGGE quello stato prima di decidere se
  bloccare. Confrontare i due path: se divergono per cwd/progetto, e' quello.
