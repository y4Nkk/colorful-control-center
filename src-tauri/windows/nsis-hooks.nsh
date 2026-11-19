; The hardware controller must exit normally so its restoration sequence can finish.
; Replace Tauri's silent force-shutdown check for both install and uninstall.
!define CCC_EXIT_CONTROLLER_RUNNING 50
!define CCC_EXIT_PROCESS_CHECK_FAILED 51

!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executablePath productName
  !insertmacro RestartManager_StartSession $R0
  ${If} $R0 == ""
    SetErrorLevel ${CCC_EXIT_PROCESS_CHECK_FAILED}
    Abort "无法检查控制台运行状态，请关闭控制台后重试。"
  ${EndIf}
  !insertmacro RestartManager_RegisterFile $R0 "${executablePath}"
  ${If} $0 != 0
    !insertmacro RestartManager_EndSession $R0
    SetErrorLevel ${CCC_EXIT_PROCESS_CHECK_FAILED}
    Abort "无法检查程序文件，请确认目录权限后重试。"
  ${EndIf}
  System::Call 'RSTRTMGR::RmGetList(p R0, *i .r1, *i .r2, p 0, *i .r3) i .r4'
  !insertmacro RestartManager_EndSession $R0
  ${If} $4 = ${ERROR_MORE_DATA}
    ${IfNot} ${Silent}
      MessageBox MB_OK|MB_ICONSTOP "请先从控制台托盘菜单选择退出，等待硬件恢复后重试。"
    ${EndIf}
    SetErrorLevel ${CCC_EXIT_CONTROLLER_RUNNING}
    Abort "控制台仍在运行，请先正常退出。"
  ${ElseIf} $4 != 0
    SetErrorLevel ${CCC_EXIT_PROCESS_CHECK_FAILED}
    Abort "无法确认控制台已退出，请稍后重试。"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  ${If} $UpdateMode <> 1
    IfFileExists "$INSTDIR\${MAINBINARYNAME}.exe" 0 login_cleanup_done
    ClearErrors
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-login' $0
    ${If} ${Errors}
    ${OrIf} $0 != 0
      ${IfNot} ${Silent}
        MessageBox MB_OK|MB_ICONSTOP "未能清理登录启动任务。卸载已取消，程序文件已保留。"
      ${EndIf}
      SetErrorLevel 2
      Abort
    ${EndIf}
    login_cleanup_done:
  ${EndIf}
!macroend
