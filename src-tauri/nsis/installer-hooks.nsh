; Tauri's default check force-closes every matching executable in silent mode.
; Stable, nightly and development copies can share that name, so installation
; and uninstallation must wait for the user to save and close those copies.
!ifmacrondef CheckIfAppIsRunning
  !error "Joker Forge requires Tauri's CheckIfAppIsRunning macro to install safely. Review the NSIS template before building."
!endif
!macroundef CheckIfAppIsRunning

!macro CheckIfAppIsRunning executableName productName
  !if "${INSTALLMODE}" == "currentUser"
    nsis_tauri_utils::FindProcessCurrentUser "${executableName}"
  !else
    nsis_tauri_utils::FindProcess "${executableName}"
  !endif
  Pop $R0
  ${If} $R0 = 0
    SetErrorLevel 1
    Abort "Save your work and close all open Joker Forge windows, including stable, nightly and development versions, then try again."
  ${EndIf}
!macroend
