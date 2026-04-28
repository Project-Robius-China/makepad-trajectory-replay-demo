#!/bin/bash
# P13.3: 预下载 OSM dark tiles 到 disk_cache, 让 demo 启动时即使断网也能显示地图.
# 演示场景适用 — community review 现场网络不可控时, 跑一次此脚本提前缓存.
#
# Usage: bash scripts/preload_tiles.sh
# Result: ~80 个 tile (~3MB) 写入 ~/Library/Caches/makepad-map/tiles/12/x/y.png
#
# 范围: cycling-track.gpx (Big Sur → Carmel California 1 公路) bbox @ zoom 12
#   lat: 35.78 - 36.56 → tile y: 1602 - 1611
#   lon: -121.94 - -121.33 → tile x: 660 - 667

set -e

CACHE_DIR="$HOME/Library/Caches/makepad-map/tiles"
ZOOM=12
X_MIN=660
X_MAX=667
Y_MIN=1602
Y_MAX=1611

# Carto Dark Matter @2x retina (与 src/map/tiles.rs:54 default tile_server 一致)
URL_PATTERN="https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}@2x.png"

mkdir -p "$CACHE_DIR/$ZOOM"

TOTAL=$(((X_MAX - X_MIN + 1) * (Y_MAX - Y_MIN + 1)))
COUNT=0
SKIPPED=0
DOWNLOADED=0
FAILED=0

echo "=== preload tiles: zoom=$ZOOM, $TOTAL tiles, target=$CACHE_DIR/$ZOOM/ ==="

for x in $(seq $X_MIN $X_MAX); do
  for y in $(seq $Y_MIN $Y_MAX); do
    COUNT=$((COUNT + 1))
    DST="$CACHE_DIR/$ZOOM/$x/$y.png"
    if [ -f "$DST" ] && [ -s "$DST" ]; then
      SKIPPED=$((SKIPPED + 1))
      printf "\r[%3d/%3d] skip %s" "$COUNT" "$TOTAL" "$x/$y.png    "
      continue
    fi
    mkdir -p "$(dirname "$DST")"
    URL=$(echo "$URL_PATTERN" | sed "s|{z}|$ZOOM|g; s|{x}|$x|g; s|{y}|$y|g")
    if curl -fsSL --max-time 15 -o "$DST" "$URL"; then
      DOWNLOADED=$((DOWNLOADED + 1))
      printf "\r[%3d/%3d] done %s" "$COUNT" "$TOTAL" "$x/$y.png    "
    else
      FAILED=$((FAILED + 1))
      rm -f "$DST"
      printf "\r[%3d/%3d] FAIL %s\n" "$COUNT" "$TOTAL" "$x/$y.png"
    fi
  done
done

echo ""
echo "=== preload 完成 ==="
echo "  下载: $DOWNLOADED"
echo "  跳过 (已存在): $SKIPPED"
echo "  失败: $FAILED"
echo "  总数: $TOTAL"
echo "缓存目录: $CACHE_DIR/$ZOOM/"
echo ""
if [ $FAILED -eq 0 ]; then
  echo "✅ 全部成功. cargo run 即可看到地图."
else
  echo "⚠️ 有 $FAILED 个 tile 失败. 检查网络后再跑一次脚本 (已下的不会重下)."
  exit 1
fi
