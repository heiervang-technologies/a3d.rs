# Demo recording for a3d — a spinning Stanford bunny rendered as ASCII 3D.
# Regenerate with: vhs assets/demo.tape   (run from the repo root)
Output assets/demo.gif

Set Shell bash
Set FontSize 18
Set Width 920
Set Height 560
Set Padding 12
Set Framerate 24
Set PlaybackSpeed 1.0

Type "a3d models/bunny.obj --color --fg 00ddff --fps 30"
Sleep 700ms
Enter
Sleep 8s
Type "q"
Sleep 500ms

# vhs emits a large (~6 MB) GIF; the committed assets/demo.gif is optimized with:
#   ffmpeg -y -i assets/demo.gif -vf "fps=12,scale=640:-1:flags=lanczos,palettegen=max_colors=48" /tmp/pal.png
#   ffmpeg -y -i assets/demo.gif -i /tmp/pal.png \
#     -filter_complex "fps=12,scale=640:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4" assets/demo.gif
