import Image from 'next/image';

export function BrandLogo({ collapsed = false, className = '' }: { collapsed?: boolean; className?: string }) {
  const name = collapsed ? 'logo-collapsed' : 'logo';
  const width = collapsed ? 40 : 254;
  const height = collapsed ? 40 : 89;
  return (
    <>
      <Image src={`/${name}.png`} alt="PartyLine" width={width} height={height} className={`object-contain dark:hidden ${className}`} priority />
      <Image src={`/${name}-dark.png`} alt="PartyLine" width={width} height={height} className={`hidden object-contain dark:block ${className}`} priority />
    </>
  );
}
