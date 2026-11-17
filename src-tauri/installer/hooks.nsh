; DevDock installer hooks (bundle.windows.nsis.installerHooks in tauri.conf.json).
; Tauri's NSIS template already closes a running DevDock before installing and
; offers "delete app data" on uninstall; these hooks only add DevDock specifics.

!macro NSIS_HOOK_POSTINSTALL
  ; App Paths entry: `devdock` then starts from Win+R and the Start menu search.
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\devdock.exe" "" "$INSTDIR\devdock.exe"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\devdock.exe" "Path" "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\devdock.exe"
!macroend
