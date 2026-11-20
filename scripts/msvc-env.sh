# Sourced in Git Bash on Windows: Visual Studio's x64 build environment (cl, link, rc, the SDK) imported into this
# shell, unless cl.exe is already on PATH (a Developer prompt, or CI's msvc-dev-cmd).
if ! command -v cl.exe >/dev/null 2>&1; then
  _vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
  _vs=$("$_vswhere" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | tr -d '\r')
  [ -n "$_vs" ] || { echo "msvc-env: no Visual Studio with the C++ tools" >&2; return 1; }
  _bat=$(cygpath -w "$_vs/VC/Auxiliary/Build/vcvars64.bat")
  _tmp=$(mktemp -u).bat
  while IFS='=' read -r _k _v; do
    case "$_k" in
      Path|PATH) export PATH="$(cygpath -u -p "$_v"):$PATH" ;;
      INCLUDE|LIB|LIBPATH|VCToolsInstallDir|VCINSTALLDIR|WindowsSdkDir|WindowsSDKVersion|UCRTVersion|UniversalCRTSdkDir|VSINSTALLDIR|WindowsSdkBinPath|WindowsSdkVerBinPath|llvmX64) export "$_k=$_v" ;;
    esac
  done <<EOT
$(printf '@call "%s" >nul\r\n@set\r\n' "$_bat" > "$_tmp" && cmd //c "$(cygpath -w "$_tmp")" | tr -d '\r')
EOT
  rm -f "$_tmp"
  unset _vswhere _vs _bat _k _v _tmp
fi
