!macro NSIS_HOOK_POSTINSTALL
  StrCpy $2 "$INSTDIR\resources\mpv-runtime"
  IfFileExists "$2\*.*" mpv_found 0
  StrCpy $2 "$INSTDIR\mpv-runtime"
  IfFileExists "$2\*.*" mpv_found mpv_done

mpv_found:
  DetailPrint "Moving libmpv runtime files into the install directory..."

  FindFirst $0 $1 "$2\*.*"
mpv_loop:
  StrCmp $1 "" mpv_done_loop
  IfFileExists "$INSTDIR\$1" 0 +2
  Delete "$INSTDIR\$1"
  Rename "$2\$1" "$INSTDIR\$1"
  FindNext $0 $1
  Goto mpv_loop

mpv_done_loop:
  FindClose $0
  RMDir "$2"

mpv_done:
!macroend