import type { Item } from './api'
import { t } from './i18n'

/** What the row says of why a message is in its tab, after "Remora:": only a guess or the user's correction. Clear words ("can you",
 * "fyi") go without saying; agents get every reason through the `remora` command. As the Mac's `SortReason.label`. */
export function whyLabel(item: Item): string | undefined {
  switch (item.reason?.code) {
    case 'unsure':
      return item.bundle.id === 'verb.reply' ? t('no clear sign, kept here to be safe') : t('no clear sign')
    case 'moved':
      return t('you put it here')
    default:
      return undefined
  }
}

/** A message Remora sorted elsewhere by its words, which the user can say waits on them. Only towards To reply, so a
 * correction never hides a request (Done does the rest). As `InboxItem::can_move_to_reply`. */
export const canMoveToReply = (item: Item) =>
  Boolean(item.reason) && item.bundle.id !== 'verb.reply' && item.bundle.id !== 'verb.awaiting'
