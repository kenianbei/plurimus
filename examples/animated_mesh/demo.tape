# The README's recording. Re-render from the workspace root:
#   vhs examples/animated_mesh/demo.tape
#   ffmpeg -y -i target/animated_mesh.mp4 -vf "fps=10,split[a][b];\
#     [a]palettegen=stats_mode=diff:max_colors=128[p];\
#     [b][p]paletteuse=dither=none:diff_mode=rectangle" \
#     examples/animated_mesh/demo.gif
# vhs's own gif dithers every frame, so a full-colour scene never compresses;
# the undithered palette above keeps the README's gif small.

Output target/animated_mesh.mp4

Set FontSize 22
Set Width 980
Set Height 600
Set Padding 16
Set Framerate 15
Set WaitTimeout 600s

Hide
Type "cargo run -q --example animated_mesh --features 3d"
Enter
Wait+Screen /▀/
# A still camera: the gif then stores only what the running fox changes.
Type " "
Sleep 4s
Show

Sleep 3s
Type "t"
Sleep 2s
Type "t"
Sleep 2s
# Braille dots every rendered pixel, which the opaque ground fills solid.
Hide
Type "t"
Sleep 200ms
Type "t"
Sleep 1s
Show
Sleep 2s
Type "e"
Sleep 3s
