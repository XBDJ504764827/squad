function Svg({ children, width = 16, height = 16, strokeWidth = 2, style }) {
  return (
    <svg width={width} height={height} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={strokeWidth} style={style}>
      {children}
    </svg>
  )
}

export default function Icon({ name, width = 16, height = 16, strokeWidth = 2, style }) {
  switch (name) {
    case 'bolt':
      return (
        <Svg width={width} height={height} strokeWidth={2.5} style={style}>
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
        </Svg>
      )
    case 'grid':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <rect x="3" y="3" width="7" height="7" rx="1" />
          <rect x="14" y="3" width="7" height="7" rx="1" />
          <rect x="3" y="14" width="7" height="7" rx="1" />
          <rect x="14" y="14" width="7" height="7" rx="1" />
        </Svg>
      )
    case 'analytics':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
        </Svg>
      )
    case 'server-manager':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <rect x="2" y="3" width="20" height="14" rx="2" />
          <path d="M8 21h8M12 17v4" />
        </Svg>
      )
    case 'globe':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="10" />
          <line x1="2" y1="12" x2="22" y2="12" />
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
        </Svg>
      )
    case 'cube':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
        </Svg>
      )
    case 'players':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
          <circle cx="9" cy="7" r="4" />
          <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
          <path d="M16 3.13a4 4 0 0 1 0 7.75" />
        </Svg>
      )
    case 'players-single':
      return (
        <Svg width={width} height={height} strokeWidth={2.5} style={style}>
          <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
          <circle cx="9" cy="7" r="4" />
        </Svg>
      )
    case 'shield':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
        </Svg>
      )
    case 'file':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
        </Svg>
      )
    case 'network':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
        </Svg>
      )
    case 'mail':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
          <polyline points="22,6 12,13 2,6" />
        </Svg>
      )
    case 'settings':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
          <path d="M3 12h3M18 12h3M12 3v3M12 18v3" />
        </Svg>
      )
    case 'sliders':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <line x1="4" y1="21" x2="4" y2="14" />
          <line x1="4" y1="10" x2="4" y2="3" />
          <line x1="12" y1="21" x2="12" y2="12" />
          <line x1="12" y1="8" x2="12" y2="3" />
          <line x1="20" y1="21" x2="20" y2="16" />
          <line x1="20" y1="12" x2="20" y2="3" />
          <line x1="2" y1="14" x2="6" y2="14" />
          <line x1="10" y1="8" x2="14" y2="8" />
          <line x1="18" y1="16" x2="22" y2="16" />
        </Svg>
      )
    case 'messages':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
          <line x1="8" y1="9" x2="16" y2="9" />
          <line x1="8" y1="13" x2="13" y2="13" />
        </Svg>
      )
    case 'radar':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="10" />
          <circle cx="12" cy="12" r="6" />
          <line x1="12" y1="2" x2="12" y2="12" />
          <line x1="12" y1="12" x2="18" y2="9" />
        </Svg>
      )
    case 'crosshair':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="7" />
          <line x1="12" y1="2" x2="12" y2="5" />
          <line x1="12" y1="19" x2="12" y2="22" />
          <line x1="2" y1="12" x2="5" y2="12" />
          <line x1="19" y1="12" x2="22" y2="12" />
          <circle cx="12" cy="12" r="2" />
        </Svg>
      )
    case 'trophy':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M8 21h8" />
          <path d="M12 17v4" />
          <path d="M8 4h8v4a4 4 0 0 1-8 0z" />
          <path d="M16 6h3a2 2 0 0 1 0 4h-3" />
          <path d="M8 6H5a2 2 0 0 0 0 4h3" />
        </Svg>
      )
    case 'file-code':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
          <polyline points="10 13 8 15 10 17" />
          <polyline points="14 13 16 15 14 17" />
        </Svg>
      )
    case 'settings-sliders':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <line x1="4" y1="21" x2="4" y2="4" />
          <line x1="12" y1="21" x2="12" y2="10" />
          <line x1="20" y1="21" x2="20" y2="7" />
          <circle cx="4" cy="9" r="2" />
          <circle cx="12" cy="15" r="2" />
          <circle cx="20" cy="12" r="2" />
        </Svg>
      )
    case 'history':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M3 12a9 9 0 1 0 3-6.7" />
          <polyline points="3 3 3 9 9 9" />
          <path d="M12 7v5l3 3" />
        </Svg>
      )
    case 'key-round':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="7.5" cy="15.5" r="3.5" />
          <path d="M10 13l8-8 3 3-2 2 2 2-2 2-2-2-2 2" />
        </Svg>
      )
    case 'menu':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <line x1="3" y1="12" x2="21" y2="12" />
          <line x1="3" y1="6" x2="21" y2="6" />
          <line x1="3" y1="18" x2="21" y2="18" />
        </Svg>
      )
    case 'search':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </Svg>
      )
    case 'bell':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
          <path d="M13.73 21a2 2 0 0 1-3.46 0" />
        </Svg>
      )
    case 'terminal':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <polyline points="4 17 10 11 4 5" />
          <line x1="12" y1="19" x2="20" y2="19" />
        </Svg>
      )
    case 'sun':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="5" />
          <line x1="12" y1="1" x2="12" y2="3" />
          <line x1="12" y1="21" x2="12" y2="23" />
          <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
          <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
          <line x1="1" y1="12" x2="3" y2="12" />
          <line x1="21" y1="12" x2="23" y2="12" />
          <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
          <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
        </Svg>
      )
    case 'moon':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </Svg>
      )
    case 'refresh':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <polyline points="23 4 23 10 17 10" />
          <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
        </Svg>
      )
    case 'arrow-left':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <line x1="19" y1="12" x2="5" y2="12" />
          <polyline points="12 19 5 12 12 5" />
        </Svg>
      )
    case 'plus':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </Svg>
      )
    case 'pencil':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M12 20h9" />
          <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z" />
        </Svg>
      )
    case 'trash':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <polyline points="3 6 5 6 21 6" />
          <path d="M19 6l-1 14H6L5 6" />
          <path d="M10 11v6M14 11v6" />
          <path d="M9 6V4h6v2" />
        </Svg>
      )
    case 'cpu':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <rect x="9" y="9" width="6" height="6" />
          <line x1="9" y1="2" x2="9" y2="4" />
          <line x1="15" y1="2" x2="15" y2="4" />
          <line x1="9" y1="20" x2="9" y2="22" />
          <line x1="15" y1="20" x2="15" y2="22" />
          <line x1="20" y1="9" x2="22" y2="9" />
          <line x1="20" y1="15" x2="22" y2="15" />
          <line x1="2" y1="9" x2="4" y2="9" />
          <line x1="2" y1="15" x2="4" y2="15" />
        </Svg>
      )
    case 'bandwidth':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <circle cx="12" cy="12" r="10" />
          <polyline points="8 12 12 16 16 12" />
          <line x1="12" y1="8" x2="12" y2="16" />
        </Svg>
      )
    case 'alert':
      return (
        <Svg width={width} height={height} strokeWidth={strokeWidth} style={style}>
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
          <line x1="12" y1="9" x2="12" y2="13" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </Svg>
      )
    case 'alert-short':
      return (
        <Svg width={width} height={height} strokeWidth={2.5} style={style}>
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
          <line x1="12" y1="9" x2="12" y2="13" />
        </Svg>
      )
    case 'check':
      return (
        <Svg width={width} height={height} strokeWidth={2.5} style={style}>
          <polyline points="20 6 9 17 4 12" />
        </Svg>
      )
    default:
      return null
  }
}
