import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import { Container } from '@mantine/core';
import { intro, itemTitle, layout, list } from '@/components/Security/Security.css';
import { band, body, CONTAINER, heading, lede, wrap } from '@/components/shared/shared.css';

const ITEMS = ['disk', 'demand', 'unlock'] as const;

export const Security: FC = () => {
  const { t } = useTranslation();

  return (
    <section aria-labelledby="security-title" className={band}>
      <Container size={CONTAINER} className={`${wrap} ${layout}`}>
        <div className={intro}>
          <h2 id="security-title" className={heading}>
            {t('security.title')}
          </h2>
          <p className={lede}>{t('security.lede')}</p>
        </div>
        <ul className={list}>
          {ITEMS.map(id => (
            <li key={id}>
              <h3 className={itemTitle}>{t(`security.items.${id}.title`)}</h3>
              <p className={body}>{t(`security.items.${id}.body`)}</p>
            </li>
          ))}
        </ul>
      </Container>
    </section>
  );
};
