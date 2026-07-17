# VHS tape for a ~25s Mycelium CLI demo.
# Render: vhs assets/demo.tape  ->  assets/demo.gif
# Runs in a throwaway /tmp project so it never touches the repo's own tracking.

Output assets/demo.gif

Set Shell "bash"
Set FontSize 18
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 45ms
Set PlaybackSpeed 1.0

# Use the release binary + a clean temp project.
Hide
Type "export PATH=$PWD/target/release:$PATH; cd $(mktemp -d); clear"
Enter
Show

Type "myc init"
Sleep 400ms
Enter
Sleep 1.5s

Type "myc epic create --title 'Launch v1'"
Sleep 300ms
Enter
Sleep 1.2s

Type "myc task create --title 'Design API' --epic 1 --priority high"
Sleep 300ms
Enter
Sleep 900ms

Type "myc task create --title 'Build backend' --epic 1 --priority critical"
Sleep 300ms
Enter
Sleep 900ms

Type "myc task create --title 'Write docs' --epic 1"
Sleep 300ms
Enter
Sleep 900ms

# Backend blocks docs.
Type "myc task link blocks --task 2 3"
Sleep 300ms
Enter
Sleep 1.2s

Type "myc list"
Sleep 300ms
Enter
Sleep 2.5s

# Start + close a task, see the tree update.
Type "myc task update 2 --status in_progress"
Sleep 300ms
Enter
Sleep 1s

Type "myc summary"
Sleep 300ms
Enter
Sleep 3s
