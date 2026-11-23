; ResinCraft Launcher's NSIS hooks (tauri.conf.json → bundle.windows.nsis.installerHooks).

; Before uninstalling: offer to save the player's chat history (src-tauri/src/chat.rs) to Documents as one compressed
; file they can open in ResinCraft again. Not for updates (the new version installs over this one) or silent uninstalls.
!macro NSIS_HOOK_PREUNINSTALL
  IfSilent rc_chats_done
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $R0
  IfErrors 0 rc_chats_done
  IfFileExists "$APPDATA\ResinCraft Launcher\chats.db" 0 rc_chats_done
  MessageBox MB_YESNO|MB_ICONQUESTION "Save your ResinCraft chat history before uninstalling?$\r$\n$\r$\nIt goes into your Documents folder as one file. Open it in ResinCraft (Inbox, Chat, Open saved chats) to get your chats back." IDNO rc_chats_done
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --export-chats "$DOCUMENTS"' $R1
  StrCmp $R1 "0" 0 rc_chats_failed
  MessageBox MB_OK|MB_ICONINFORMATION "Your chat history was saved in your Documents folder (ResinCraft-chats-…)."
  Goto rc_chats_done
  rc_chats_failed:
  StrCmp $R1 "3" rc_chats_done
  MessageBox MB_OK|MB_ICONEXCLAMATION "Your chat history couldn't be saved. Uninstalling goes on; your chats are still in $APPDATA\ResinCraft Launcher unless you delete that folder."
  rc_chats_done:
!macroend
