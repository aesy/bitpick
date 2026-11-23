import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import { about, brand, footer, link, links as linkList, row } from '@/components/Footer/Footer.css';
import { CONTAINER, wrap } from '@/components/shared/shared.css';
import { links } from '@/lib/links';

const LINKS = [
  { href: links.source, label: 'footer.source' },
  { href: links.releases, label: 'footer.releases' },
  { href: links.issues, label: 'footer.issues' },
] as const;

export const Footer: FC = () => {
  const { t } = useTranslation();

  return (
    <footer className={footer}>
      <Container size={CONTAINER} className={`${wrap} ${row}`}>
        <div className={about}>
          <span className={brand}>
            <img src={links.logo} alt="" width={22} height={22} />
            bitpick
          </span>
          <span>{t('footer.license')}</span>
        </div>
        <nav aria-label={t('footer.label')} className={linkList}>
          {LINKS.map(({ href, label }) => (
            <a key={label} className={link} href={href}>
              {t(label)}
            </a>
          ))}
        </nav>
      </Container>
    </footer>
  );
};
