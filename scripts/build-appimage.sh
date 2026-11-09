#!/usr/bin/env bash
# 构建 Pomodoro AppImage。
#
# 为什么不用 `npm run tauri build` 直接出包：
# tauri 的打包器会从 github.com/tauri-apps/binary-releases 下载 linuxdeploy，
# 在当前网络环境下这个下载会被替换成一个无关的 Electron 应用（tauri 仍报告
# "成功"但不产出任何文件）。本脚本复用 tauri 组装好的 AppDir，只手动完成最后
# 一步：mksquashfs + 拼接官方 type2-runtime。
#
# 用法:  scripts/build-appimage.sh
# 产物:  src-tauri/target/release/bundle/appimage/Pomodoro_<版本>_amd64.AppImage
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TAURI_DIR="$ROOT/src-tauri"
BUNDLE_DIR="$TAURI_DIR/target/release/bundle/appimage"
APPDIR="$BUNDLE_DIR/Pomodoro.AppDir"
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
RUNTIME="$CACHE_DIR/runtime-x86_64"

for tool in npm cargo mksquashfs python3 curl; do
  command -v "$tool" >/dev/null || { echo "缺少依赖: $tool"; exit 1; }
done

echo "==> 1/3 编译前端与 Rust（本机 bundler 会静默失败，无碍，AppDir 已由它组装好）"
npm run tauri build -- --bundles appimage

BIN="$TAURI_DIR/target/release/pomodoro"
[ -x "$BIN" ] || { echo "编译产物不存在: $BIN"; exit 1; }

echo "==> 2/3 检查 AppDir"
if [ ! -x "$APPDIR/usr/bin/pomodoro" ]; then
  mkdir -p "$APPDIR/usr/bin"
  cp -f "$BIN" "$APPDIR/usr/bin/pomodoro"
fi
if [ ! -e "$APPDIR/AppRun" ]; then
  # tauri 正常打包时会把 AppRun 放进 AppDir；缺失时（例如全新目录）从上游补齐。
  APPRUN="$CACHE_DIR/AppRun-x86_64"
  if [ ! -f "$APPRUN" ]; then
    mkdir -p "$CACHE_DIR"
    curl -sSL -o "$APPRUN" \
      "https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64"
  fi
  # 校验：真 AppRun 是几十 KB 的 ELF，被网络替换成别的东西时直接报错。
  SIZE=$(stat -c%s "$APPRUN")
  [ "$SIZE" -lt 200000 ] || { echo "AppRun 下载可疑（$SIZE 字节），请检查 $APPRUN"; exit 1; }
  cp "$APPRUN" "$APPDIR/AppRun"
  chmod +x "$APPDIR/AppRun"
  cp "$TAURI_DIR/icons/icon.png" "$APPDIR/.DirIcon"
  cp "$TAURI_DIR/icons/icon.png" "$APPDIR/Pomodoro.png"
fi

echo "==> 3/3 获取 type2-runtime 并合成 AppImage"
if [ ! -f "$RUNTIME" ]; then
  mkdir -p "$CACHE_DIR"
  curl -sSL -o "$RUNTIME" \
    "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64"
fi
# 校验：type2 runtime 在偏移 8 处有 AI\x02 魔数，体积约 1MB。
python3 - "$RUNTIME" <<'EOF'
import sys
data = open(sys.argv[1], "rb").read()
ok = 500_000 < len(data) < 3_000_000 and data[:4] == b"\x7fELF" and data[8:11] == b"AI\x02"
if not ok:
    sys.exit(f"runtime 文件可疑: size={len(data)} magic={data[8:11]!r}，请删除后重试")
EOF

VERSION=$(grep -m1 '^version' "$TAURI_DIR/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')
OUT="$BUNDLE_DIR/Pomodoro_${VERSION}_amd64.AppImage"
rm -f "$OUT" "$BUNDLE_DIR/pomodoro.squashfs"
mksquashfs "$APPDIR" "$BUNDLE_DIR/pomodoro.squashfs" \
  -all-root -noappend -no-recovery -comp zstd -Xcompression-level 19 -b 1M >/dev/null
# 注意：新版 type2 runtime 只支持 zstd/zlib，不要改回 xz。
cat "$RUNTIME" "$BUNDLE_DIR/pomodoro.squashfs" > "$OUT"
chmod +x "$OUT"
rm -f "$BUNDLE_DIR/pomodoro.squashfs"

echo "完成: $OUT"
