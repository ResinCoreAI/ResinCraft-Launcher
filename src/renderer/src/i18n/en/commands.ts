// Environment variables and custom commands (Settings → Commands, and each instance's settings).
export default {
  'commands.nav': 'Commands',
  'commands.lead': 'Environment variables for the game, and your own commands that run when it starts and after it closes.',
  'commands.section': 'Commands and environment',
  'commands.warning': 'Commands run on this PC with your user account, exactly as written. Only add commands you understand and trust.',
  'commands.env': 'Environment variables',
  'commands.env.hint': 'Passed to every game and to the commands below.',
  'commands.env.instanceHint': 'Added to the ones in Settings. A name set here replaces the one there.',
  'commands.env.name': 'Name',
  'commands.env.value': 'Value',
  'commands.env.add': 'Add variable',
  'commands.env.remove': 'Remove {name}',
  'commands.env.badName': "Use letters, digits and _, and don't start with a digit.",
  'commands.env.reserved': 'Names starting with INST_ are set by ResinCraft.',
  'commands.env.duplicate': 'This name is already in the list.',
  'commands.preLaunch': 'Pre-launch command',
  'commands.preLaunch.hint': "Runs before Java starts. If it ends with an error, the game doesn't start. It's stopped after 5 minutes, or when you cancel it.",
  'commands.wrapper': 'Wrapper command',
  'commands.wrapper.hint':
    'The game runs through this, for example prime-run or obs-gamecapture. Put {java} where the Java command goes, or leave it out to add it at the end.',
  'commands.postExit': 'Post-exit command',
  'commands.postExit.hint': 'Runs after the game closes. $INST_EXIT_CODE holds its exit code.',
  'commands.variables':
    'Commands run through {shell} in the game folder and can use $INST_NAME, $INST_ID, $INST_DIR, $INST_MC_DIR, $INST_JAVA, $INST_JAVA_ARGS and your variables.',
  'commands.own': "Use this instance's own commands",
  'commands.own.hint': 'Replaces the commands from Settings for this instance.'
}
