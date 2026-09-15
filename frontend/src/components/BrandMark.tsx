'use client';

/**
 * The gcrdings mark, drawn as vector.
 *
 * The source artwork is a 1.1 MB SVG wrapping a raster, which is right for an
 * app icon and wrong for a control that renders at 20 px in a sidebar. This is
 * the same identity — a G whose counter holds a record light — authored as
 * geometry so it stays sharp at every size and can follow the theme.
 *
 * The red is reserved. It is the record light, and using it for anything else
 * would cost it that meaning.
 */
export const BRAND = {
  record: '#FF2C30',
  ink: '#FEFEFE',
  ground: '#000000',
} as const;

interface BrandMarkProps {
  size?: number;
  /** Draws the rounded-square ground behind the mark, as on the app icon. */
  withGround?: boolean;
  className?: string;
  title?: string;
}

export function BrandMark({
  size = 32,
  withGround = true,
  className,
  title = 'gcrdings',
}: BrandMarkProps) {
  // Scoped per instance: two marks on one page would otherwise share gradient
  // ids and the second would inherit the first's fill.
  const uid = `brand-${size}-${withGround ? 'g' : 'p'}`;

  return (
    <svg
      viewBox="0 0 64 64"
      width={size}
      height={size}
      className={className}
      role="img"
      aria-label={title}
    >
      <defs>
        {/* The specular streak from the artwork: white at low opacity across a
            135° diagonal, peaking narrowly so it reads as a highlight rather
            than a wash. */}
        <linearGradient id={`${uid}-sheen`} x1="0" y1="1" x2="1" y2="0">
          <stop offset="0%" stopColor="#FFFFFF" stopOpacity="0" />
          <stop offset="42%" stopColor="#FFFFFF" stopOpacity="0" />
          <stop offset="50%" stopColor="#FFFFFF" stopOpacity="0.30" />
          <stop offset="58%" stopColor="#FFFFFF" stopOpacity="0" />
          <stop offset="100%" stopColor="#FFFFFF" stopOpacity="0" />
        </linearGradient>

        <radialGradient id={`${uid}-glow`}>
          <stop offset="0%" stopColor={BRAND.record} stopOpacity="0.55" />
          <stop offset="100%" stopColor={BRAND.record} stopOpacity="0" />
        </radialGradient>
      </defs>

      {withGround && (
        <>
          <rect x="0" y="0" width="64" height="64" rx="14.5" fill={BRAND.ground} />
          <rect x="0" y="0" width="64" height="64" rx="14.5" fill={`url(#${uid}-sheen)`} />
        </>
      )}

      {/* The G: an arc open on the right, closed by a bar that stops short of
          centre so the record light floats clear in the counter, as it does in
          the artwork. Stroked rather than filled so weight scales. */}
      <path
        d="M 44.5 20.5 A 15.5 15.5 0 1 0 47.5 32 L 40.5 32"
        fill="none"
        stroke={withGround ? BRAND.ink : 'currentColor'}
        strokeWidth="7"
        strokeLinecap="butt"
      />

      {withGround && <circle cx="32" cy="32" r="11" fill={`url(#${uid}-glow)`} />}

      <circle
        cx="32"
        cy="32"
        r="5.5"
        fill={withGround ? BRAND.record : 'currentColor'}
        opacity={withGround ? 1 : 0.9}
      />
    </svg>
  );
}
