#!/bin/bash
# Film the shop's upgrade demo clips: each upgrade's run without it, then
# with it, cropped to where it shows, labelled, stitched into a looping GIF.
# Usage: make_clips.sh <out_dir> [ids...]
set -e
GAME=/Users/iz3xypug/workspace/synful_quartz/ball_swing_game
OUT=$1; shift
WORK=$OUT/work; mkdir -p "$WORK"
cd "$GAME"
# id | before ranks | after ranks | extra flags | env | from | to | crop (w:h:x:y of 640x360)
spec() {
  case $1 in
    secondwind) echo "secondwind:0|secondwind:1|--boss-kind colossus|HEADLESS_COLOSSUS_HEAD=1 HEADLESS_FREE_FALLS=1 HEADLESS_BOSS_HEARTS=1 HEADLESS_CAM_BOSS=0.5 HEADLESS_DEATH_HOLD=120|812|948|640:360:0:0" ;;
  esac
}
H=./target/release/headless
LBL=$WORK/labels; mkdir -p "$LBL"
[ -f "$LBL/before.png" ] || HEADLESS_LABEL="BEFORE@$LBL/before.png" $H
[ -f "$LBL/after.png" ]  || HEADLESS_LABEL="AFTER@$LBL/after.png" $H
for id in "$@"; do
  IFS='|' read -r b a flags envs from to crop <<< "$(spec $id)"
  frames=$((to + 4))
  for side in before after; do
    ranks=$b; [ $side = after ] && ranks=$a
    d=$WORK/$id/$side; rm -rf "$d"; mkdir -p "$d"
    e=(); [ "$envs" != "-" ] && e=($envs)
    env "${e[@]}" HEADLESS_PERM=$ranks \
      HEADLESS_SHOT_W=640 HEADLESS_SHOT_DIR="$d" HEADLESS_SHOT_EVERY=4 HEADLESS_SHOT_FROM=$from \
      $H --episodes 1 --frames $frames $flags >/dev/null 2>&1 || true
  done
  # The first shot is laid out before the frame size settles: drop it.
  for side in before after; do rm -f "$(ls "$WORK/$id/$side"/*.png | head -1)"; done
  IFS=':' read -r cw ch cx cy <<< "$crop"
  seg() { # dir label -> mp4
    ffmpeg -y -loglevel error -framerate 15 -pattern_type glob -i "$1/*.png" -i "$2" \
      -filter_complex "[0]crop=$cw:$ch:$cx:$cy,scale=480:270:flags=lanczos[v];[1]scale=-1:34[l];[v][l]overlay=(W-w)/2:H-h-10" \
      -c:v libx264 -pix_fmt yuv420p "$3"
  }
  seg "$WORK/$id/before" "$LBL/before.png" "$WORK/$id/before.mp4"
  seg "$WORK/$id/after" "$LBL/after.png" "$WORK/$id/after.mp4"
  dur=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$WORK/$id/before.mp4")
  off=$(python3 -c "print(max(0.1, $dur - 0.25))")
  ffmpeg -y -loglevel error -i "$WORK/$id/before.mp4" -i "$WORK/$id/after.mp4" \
    -filter_complex "[0][1]xfade=transition=fade:duration=0.25:offset=$off,split[a][b];[a]palettegen=max_colors=96[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" \
    -loop 0 "$OUT/$id.gif"
  echo "$id -> $(du -h "$OUT/$id.gif" | cut -f1)"
done
