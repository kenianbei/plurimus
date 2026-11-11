# The README's recording. Re-render from the workspace root:
#   vhs examples/pong/demo.tape

Output examples/pong/demo.gif

Set Shell bash
Set FontSize 18
Set Width 980
Set Height 600
Set Padding 16
Set Framerate 30
Set WaitTimeout 600s

Hide
Type "cargo run -q --release --example pong --features widgets,2d -- --demo"
Enter
Wait+Screen /0 : 0/
Show

Sleep 10s
