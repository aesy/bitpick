import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import { card, featuresLede, grid, icon } from '@/components/Features/Features.css';
import {
  BoltIcon,
  ClockIcon,
  LockIcon,
  PenIcon,
  SearchIcon,
  StackIcon,
} from '@/components/shared/icons';
import {
  band,
  body,
  CONTAINER,
  cardTitle,
  heading,
  lede,
  section,
  wrap,
} from '@/components/shared/shared.css';
import { colors } from '@/theme/colors';

const FEATURES = [
  { id: 'fast', Icon: BoltIcon },
  { id: 'fzf', Icon: SearchIcon },
  { id: 'fields', Icon: LockIcon },
  { id: 'clipboard', Icon: ClockIcon },
  { id: 'profiles', Icon: StackIcon },
  { id: 'edit', Icon: PenIcon },
] as const;

export const Features: FC = () => {
  const { t } = useTranslation();

  return (
    <section id="features" className={band}>
      <Container size={CONTAINER} className={`${wrap} ${section}`}>
        <h2 className={heading}>{t('features.title')}</h2>
        <p className={`${lede} ${featuresLede}`}>{t('features.lede')}</p>
        <div className={grid}>
          {FEATURES.map(({ id, Icon }) => (
            <article key={id} className={card}>
              <span className={icon}>
                <Icon size={28} color={colors.accent} />
              </span>
              <h3 className={cardTitle}>{t(`features.items.${id}.title`)}</h3>
              <p className={body}>{t(`features.items.${id}.body`)}</p>
            </article>
          ))}
        </div>
      </Container>
    </section>
  );
};
