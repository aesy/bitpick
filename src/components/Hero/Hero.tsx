import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import { Terminal } from '@/components/Terminal/Terminal';
import { copy, ctas, hero, heroLede, note, screenColumn, title } from '@/components/Hero/Hero.css';
import { CONTAINER, outlineButton, primaryButton, wrap } from '@/components/shared/shared.css';
import { links } from '@/lib/links';

export const Hero: FC = () => {
  const { t } = useTranslation();

  return (
    <Container component="section" size={CONTAINER} className={`${wrap} ${hero}`}>
      <div className={copy}>
        <h1 className={title}>{t('hero.title')}</h1>
        <p className={heroLede}>{t('hero.lede')}</p>
        <div className={ctas}>
          <a className={primaryButton} href="#install">
            {t('hero.install')}
          </a>
          <a className={outlineButton} href={links.source}>
            {t('hero.source')}
          </a>
        </div>
        <p className={note}>{t('hero.note')}</p>
      </div>
      <div className={screenColumn}>
        <Terminal />
      </div>
    </Container>
  );
};
