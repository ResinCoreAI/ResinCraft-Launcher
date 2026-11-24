// Share a Store item into a chat (a friend's or a group's): it shows there as a card with its price. The window is
// "Send to chat" (SendToChat.tsx).
import { SendToChatModal } from './SendToChat'

export default function ShareModal({ product, name, onClose }: { product: string; name: string; onClose: () => void }) {
  return <SendToChatModal item={{ kind: 'store', title: name, product }} onClose={onClose} />
}
