// A server's log browser (src-tauri/src/server/logs.rs).

export interface LogFile {
  /** What to ask for it by: "logs/latest.log". */
  path: string
  name: string
  /** In crash-reports/. */
  crash: boolean
  /** On disk, so packed for a .gz. */
  size: number
  modified: number
}

export interface LogFileLine {
  /** Counted from 1. */
  line: number
  text: string
  level: 'debug' | 'info' | 'warn' | 'error'
}

export interface LogPage {
  lines: LogFileLine[]
  /** There are lines after these. */
  more: boolean
  /** The file ends early or isn't what its name says: these are the lines that could be read. */
  damaged: boolean
  /** The engine's read limit stopped it before the end of the file. */
  capped: boolean
}

export interface LogHit extends LogFileLine {
  path: string
}

export interface LogSearch {
  hits: LogHit[]
  /** There are more matches than `hits`. */
  more: boolean
  /** The engine's read limit stopped it: some files or lines weren't looked at. */
  capped: boolean
  /** Files that could only be read in part, or not at all. */
  damaged: string[]
}

export interface LogQuery {
  /** One file; all of them when absent. */
  path?: string
  /** Plain text, any case. Empty needs `problems`. */
  query: string
  /** Warnings and errors only. */
  problems: boolean
}
