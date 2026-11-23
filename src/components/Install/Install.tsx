import type { FC } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import {
  card,
  cardHeading,
  firstRun,
  firstRunCommands,
  firstRunTitle,
  grid,
  installLede,
  releasesButton,
  snippet,
} from '@/components/Install/Install.css';
import { Commands } from '@/components/shared/Commands';
import {
  body,
  CONTAINER,
  heading,
  lede,
  primaryButton,
  section,
  wrap,
} from '@/components/shared/shared.css';
import { links } from '@/lib/links';

const ARCH = [
  'git clone https://github.com/aesy/bitpick',
  'cd bitpick/app/pkg/arch',
  'makepkg -si',
];

const CARGO = ['cargo install bitpick-cli'];

const FIRST_RUN = ['rbw config set email you@example.com', 'rbw login', 'bitpick'];

export const Install: FC = () => {
  const { t } = useTranslation();

  return (
    <Container component="section" id="install" size={CONTAINER} className={`${wrap} ${section}`}>
      <h2 className={heading}>{t('install.title')}</h2>
      <p className={`${lede} ${installLede}`}>
        <Trans
          i18nKey="install.lede"
          components={{ rbw: <a href={links.rbw} />, fzf: <a href={links.fzf} /> }}
        />
      </p>
      <div className={grid}>
        <article className={card}>
          <h3 className={cardHeading}>{t('install.releases.title')}</h3>
          <p className={body}>{t('install.releases.body')}</p>
          <a className={`${primaryButton} ${releasesButton}`} href={links.releases}>
            {t('install.releases.link')}
          </a>
        </article>
        <article className={card}>
          <h3 className={cardHeading}>{t('install.arch.title')}</h3>
          <p className={body}>{t('install.arch.body')}</p>
          <Commands lines={ARCH} className={snippet} />
        </article>
        <article className={card}>
          <h3 className={cardHeading}>{t('install.cargo.title')}</h3>
          <p className={body}>{t('install.cargo.body')}</p>
          <Commands lines={CARGO} className={snippet} />
        </article>
      </div>
      <div className={firstRun}>
        <h3 className={firstRunTitle}>{t('install.firstRun')}</h3>
        <Commands lines={FIRST_RUN} className={firstRunCommands} />
      </div>
    </Container>
  );
};
