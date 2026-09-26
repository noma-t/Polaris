import { useId, type CSSProperties } from "react";

interface PolarisLogoProps {
  className?: string;
  style?: CSSProperties;
}

/** 星座 + 北極星のロゴマーク。線色・星色は CSS 変数 --logo-line / --logo-star */
export function PolarisLogo({ className, style }: PolarisLogoProps) {
  const uid = useId();
  const glowId = `${uid}-glow`;
  const beamId = `${uid}-beam`;
  return (
    <svg className={className} style={style} viewBox="160 200 735 640" aria-hidden="true">
      <defs>
        <radialGradient id={glowId} cx="758" cy="340" r="95" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="var(--logo-star)" stopOpacity="0.9" />
          <stop offset="0.4" stopColor="var(--logo-star)" stopOpacity="0.35" />
          <stop offset="1" stopColor="var(--logo-star)" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={beamId} x1="645" y1="470" x2="718" y2="392" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="var(--logo-line)" />
          <stop offset="1" stopColor="var(--logo-star)" />
        </linearGradient>
      </defs>
      <g fill="none" stroke="var(--logo-line)" strokeWidth="13" strokeLinecap="round" strokeLinejoin="round">
        <line x1="292" y1="556" x2="214" y2="664" />
        <line x1="236" y1="714" x2="508" y2="800" />
        <line x1="610" y1="548" x2="562" y2="772" />
        <path d="M358 540 C420 548 470 600 462 660 C458 695 440 712 412 714" />
        <path d="M468 655 C490 615 530 600 572 600" />
        <path d="M625 605 C670 618 700 650 735 665 C755 672 770 668 778 665" />
        <path d="M406 606 C410 620 400 632 386 633" />
        <path d="M346 666 C350 682 340 692 325 692" />
      </g>
      <line x1="645" y1="470" x2="718" y2="392" stroke={`url(#${beamId})`} strokeWidth="9" strokeLinecap="round" />
      <g fill="var(--logo-line)">
        <circle cx="315" cy="522" r="26" />
        <circle cx="190" cy="697" r="26" />
        <circle cx="625" cy="497" r="26" />
        <circle cx="550" cy="807" r="26" />
      </g>
      <circle cx="758" cy="340" r="95" fill={`url(#${glowId})`} />
      <polygon
        fill="var(--logo-star)"
        points="758,205 767,326 830,268 773,333 890,340 773,347 828,410 767,354 760,475 750,354 700,405 743,347 628,340 743,333 690,272 750,326"
      />
    </svg>
  );
}
