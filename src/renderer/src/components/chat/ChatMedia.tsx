// Pictures, voice messages and files in chat (kept on the relay's Telegram drive, fetched through the relay and kept on
// this PC after the first time), profile pictures with a status dot, and the voice recorder for the message box.
import { useEffect, useRef, useState } from 'react'
import { Download, FileArchive, FileText, Mic, Package, Pause, Play, Square, X } from 'lucide-react'
import type { ChatFile, ChatStatus } from '@shared/types'
import { api } from '../../api'
import { avatarOf, nameOf, statusOf } from '../../chatState'
import { useStore } from '../../store'
import { t } from '../../i18n'
import { Modal } from '../ui'
import PlayerHead from '../PlayerHead'

// ----- Files the page shows, as blob URLs (one fetch per file) -----
const blobs = new Map<string, Promise<string>>()
export function blobUrl(id: string): Promise<string> {
  let p = blobs.get(id)
  if (!p) {
    p = api.chat.fileData(id).then(({ mime, data }) => {
      const bin = atob(data)
      const bytes = new Uint8Array(bin.length)
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
      return URL.createObjectURL(new Blob([bytes], { type: mime }))
    })
    p.catch(() => blobs.delete(id))
    blobs.set(id, p)
  }
  return p
}
export function useBlob(id: string | null | undefined): string | null {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!id) {
      setUrl(null)
      return
    }
    let live = true
    void blobUrl(id).then(
      (u) => live && setUrl(u),
      () => live && setUrl(null)
    )
    return () => {
      live = false
    }
  }, [id])
  return url
}

export function FileImage({ id, alt = '', className, style, onClick }: { id: string; alt?: string; className?: string; style?: React.CSSProperties; onClick?: () => void }) {
  const url = useBlob(id)
  if (!url) return <span className={`chat-img-wait ${className ?? ''}`} style={style} aria-hidden />
  return <img src={url} alt={alt} className={className} style={style} onClick={onClick} draggable={false} />
}

/** A player's picture: their profile picture, else their Minecraft head; with a status dot when asked. */
export function ChatAvatar({ uuid, size = 32, dot, status }: { uuid: string; size?: number; dot?: boolean; status?: ChatStatus }) {
  const pic = avatarOf(uuid)
  const s = status ?? statusOf(uuid)
  const face = pic ? <FileImage id={pic} className="chat-avatar-img" style={{ width: size, height: size }} /> : <PlayerHead who={uuid} name={nameOf(uuid)} size={size} className="chat-head" />
  if (!dot) return face
  return (
    <span className="chat-head-wrap">
      {face}
      <span className={`chat-presence ${s}`} aria-label={t(`chat.status.${s}` as never)} />
    </span>
  )
}

const size = (n: number) => (n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)} MB` : n >= 1000 ? `${Math.round(n / 1000)} KB` : `${n} B`)
const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`

function VoicePlayer({ f }: { f: ChatFile }) {
  const url = useBlob(f.id)
  const el = useRef<HTMLAudioElement>(null)
  const [playing, setPlaying] = useState(false)
  const [at, setAt] = useState(0)
  const length = f.duration ?? 0
  return (
    <div className="chat-voice">
      <button
        className="btn primary icon sm"
        aria-label={playing ? t('chat.voice.pause') : t('chat.voice.play')}
        disabled={!url}
        onClick={() => {
          const a = el.current
          if (!a) return
          if (a.paused) void a.play()
          else a.pause()
        }}
      >
        {playing ? <Pause /> : <Play />}
      </button>
      <span className="chat-voice-bar" aria-hidden>
        <span style={{ width: `${length ? Math.min(100, (at / length) * 100) : 0}%` }} />
      </span>
      <span className="chat-time">{clock(playing || at ? at : length)}</span>
      {url && <audio ref={el} src={url} preload="metadata" onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => (setPlaying(false), setAt(0))} onTimeUpdate={(e) => setAt(e.currentTarget.currentTime)} />}
    </div>
  )
}

function FileCard({ f }: { f: ChatFile }) {
  const { toast, instances } = useStore()
  const [adding, setAdding] = useState(false)
  const mod = /\.jar$/i.test(f.name)
  const save = async () => {
    try {
      const where = await api.chat.fileSave(f.id, f.name)
      if (where) toast(t('chat.file.saved'), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  const addTo = async (instanceId: string, name: string) => {
    setAdding(false)
    try {
      const path = await api.chat.fileLocal(f.id)
      await api.files.import({ kind: 'instance', id: instanceId }, 'mods', [path])
      toast(t('chat.file.addedTo', { name: f.name, instance: name }), 'success')
    } catch (e) {
      toast((e as Error).message, 'error')
    }
  }
  return (
    <div className="chat-embed chat-file">
      <span className="chat-embed-icon" aria-hidden>
        {mod ? <Package /> : /\.(zip|mrpack|rar|7z)$/i.test(f.name) ? <FileArchive /> : <FileText />}
      </span>
      <span className="chat-embed-text">
        <b className="ellipsis">{f.name}</b>
        <span className="hint">{size(f.size)}</span>
      </span>
      {mod && instances.length > 0 && (
        <button className="btn sm" onClick={() => setAdding(true)}>
          {t('chat.file.addToInstance')}
        </button>
      )}
      <button className="btn ghost icon sm" title={t('chat.file.save')} aria-label={t('chat.file.save')} onClick={() => void save()}>
        <Download />
      </button>
      {adding && (
        <Modal title={t('chat.file.pickInstance', { name: f.name })} onClose={() => setAdding(false)}>
          <div className="chat-pick-list">
            {instances.map((i) => (
              <button key={i.id} className="chat-pick-row" onClick={() => void addTo(i.id, i.name)}>
                <span className="grow ellipsis">{i.name}</span>
                <span className="hint">
                  {i.mcVersion} {i.loader ?? ''}
                </span>
              </button>
            ))}
          </div>
        </Modal>
      )}
    </div>
  )
}

/** A message's files: pictures in a grid (click: full size), voice messages and file cards. */
export function MessageFiles({ files }: { files: ChatFile[] }) {
  const [big, setBig] = useState<ChatFile | null>(null)
  const { toast } = useStore()
  const pics = files.filter((f) => f.kind === 'image')
  return (
    <div className="chat-files">
      {pics.length > 0 && (
        <div className={`chat-pics n${Math.min(pics.length, 4)}`}>
          {pics.map((f) => (
            <FileImage key={f.id} id={f.id} alt={f.name} className="chat-pic" onClick={() => setBig(f)} style={pics.length === 1 && f.w && f.h ? { aspectRatio: `${f.w} / ${f.h}` } : undefined} />
          ))}
        </div>
      )}
      {files.filter((f) => f.kind === 'voice').map((f) => (
        <VoicePlayer key={f.id} f={f} />
      ))}
      {files.filter((f) => f.kind === 'file').map((f) => (
        <FileCard key={f.id} f={f} />
      ))}
      {big && (
        <div className="chat-lightbox" role="dialog" aria-label={big.name} onClick={() => setBig(null)}>
          <FileImage id={big.id} alt={big.name} className="chat-lightbox-img" />
          <div className="chat-lightbox-bar" onClick={(e) => e.stopPropagation()}>
            <span className="ellipsis">{big.name}</span>
            <button
              className="btn sm"
              onClick={() =>
                void api.chat.fileSave(big.id, big.name).then(
                  (w) => w && toast(t('chat.file.saved'), 'success'),
                  (e) => toast((e as Error).message, 'error')
                )
              }
            >
              <Download /> {t('chat.file.save')}
            </button>
            <button className="btn ghost icon sm" aria-label={t('common.close')} onClick={() => setBig(null)}>
              <X />
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

/** Records a voice message with the microphone: start, then send or cancel. At most 5 minutes. */
export function VoiceRecorder({ onDone, onCancel }: { onDone: (blob: Blob, seconds: number) => void; onCancel: () => void }) {
  const { toast } = useStore()
  const [secs, setSecs] = useState(0)
  const rec = useRef<MediaRecorder | null>(null)
  const chunks = useRef<Blob[]>([])
  const started = useRef(0)
  const cancelled = useRef(false)
  useEffect(() => {
    let stream: MediaStream | null = null
    let timer = 0
    void navigator.mediaDevices
      .getUserMedia({ audio: true })
      .then((s) => {
        stream = s
        const type = MediaRecorder.isTypeSupported('audio/webm;codecs=opus') ? 'audio/webm;codecs=opus' : ''
        const r = new MediaRecorder(s, type ? { mimeType: type } : undefined)
        rec.current = r
        r.ondataavailable = (e) => e.data.size && chunks.current.push(e.data)
        r.onstop = () => {
          stream?.getTracks().forEach((tr) => tr.stop())
          if (cancelled.current) return
          const blob = new Blob(chunks.current, { type: 'audio/webm' })
          onDone(blob, (Date.now() - started.current) / 1000)
        }
        started.current = Date.now()
        r.start(250)
        timer = window.setInterval(() => {
          const s2 = (Date.now() - started.current) / 1000
          setSecs(s2)
          if (s2 >= 300) r.stop()
        }, 250)
      })
      .catch(() => {
        toast(t('chat.voice.noMic'), 'error')
        onCancel()
      })
    return () => {
      window.clearInterval(timer)
      if (rec.current?.state === 'recording') {
        cancelled.current = true
        rec.current.stop()
      }
      stream?.getTracks().forEach((tr) => tr.stop())
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  return (
    <div className="chat-recording">
      <span className="chat-rec-dot" aria-hidden />
      <Mic size={16} aria-hidden />
      <span className="grow">{t('chat.voice.recording', { time: clock(secs) })}</span>
      <button
        className="btn ghost sm"
        onClick={() => {
          cancelled.current = true
          rec.current?.stop()
          onCancel()
        }}
      >
        {t('common.cancel')}
      </button>
      <button className="btn primary sm" disabled={secs < 0.5} onClick={() => rec.current?.stop()}>
        <Square size={14} /> {t('chat.voice.send')}
      </button>
    </div>
  )
}
