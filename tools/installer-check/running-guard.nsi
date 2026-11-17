Unicode true
!include LogicLib.nsh
!include Win\RestartManager.nsh
!define /ifndef ERROR_MORE_DATA 234
; Stand in for the macro that Tauri defines before loading the project's hooks.
!macro CheckIfAppIsRunning executablePath productName
  !error "The project must replace the default force-shutdown check."
!macroend
!include "${HOOKS}"
Name "CCC isolated running guard"
OutFile "${OUTPUT}"
RequestExecutionLevel user
SilentInstall silent
Section
  !insertmacro CheckIfAppIsRunning "${CONTROLLER}" "CCC fixture"
  SetErrorLevel 0
SectionEnd
