# StepList

A setup that takes several stops in Android's Settings: done steps collapse to one `success` line, the current one opens with its instructions and button, later ones wait.

**Provide** `steps` (`{title, body?, done?}`), `current`.

- Each step names the exact path on the phone ("Settings → About phone → Software information"), using Samsung's names on Samsung.
- EchoConnect checks each step itself and moves on; the person never ticks a box.
