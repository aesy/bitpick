import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import { GithubIcon } from '@/components/shared/icons';
import {
  actions,
  brand,
  brandLogo,
  githubButton,
  link,
  nav,
  navbar,
} from '@/components/Navbar/Navbar.css';
import { CONTAINER, wrap } from '@/components/shared/shared.css';
import { links } from '@/lib/links';

const SECTIONS = [
  { id: 'features', label: 'nav.features' },
  { id: 'keys', label: 'nav.keybindings' },
  { id: 'install', label: 'nav.install' },
] as const;

export const Navbar: FC = () => {
  const { t } = useTranslation();

  return (
    <header className={navbar}>
      <Container size={CONTAINER} className={wrap}>
        <nav aria-label={t('nav.label')} className={nav}>
          <a className={brand} href="#top">
            <img className={brandLogo} src={links.logo} alt="" width={32} height={32} />
            <span>bitpick</span>
          </a>
          <div className={actions}>
            {SECTIONS.map(({ id, label }) => (
              <a key={id} className={link} href={`#${id}`}>
                {t(label)}
              </a>
            ))}
            <a className={githubButton} href={links.source}>
              <GithubIcon />
              <span>{t('nav.github')}</span>
            </a>
          </div>
        </nav>
      </Container>
    </header>
  );
};
