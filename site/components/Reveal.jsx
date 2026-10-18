'use client'

import { useEffect } from 'react'
import { usePathname } from 'next/navigation'

/** Fades `.reveal` elements in once as they scroll into view (CSS skips it under reduced motion). */
export default function Reveal() {
  const path = usePathname()
  useEffect(() => {
    const items = document.querySelectorAll('.reveal:not(.in)')
    if (!('IntersectionObserver' in window)) {
      items.forEach((el) => el.classList.add('in'))
      return
    }
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue
          e.target.classList.add('in')
          io.unobserve(e.target)
        }
      },
      { rootMargin: '0px 0px -8% 0px', threshold: 0.08 }
    )
    items.forEach((el) => io.observe(el))
    return () => io.disconnect()
  }, [path])
  return null
}
