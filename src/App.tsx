import { Features } from '@/components/Features/Features';
import { Footer } from '@/components/Footer/Footer';
import { Hero } from '@/components/Hero/Hero';
import { Install } from '@/components/Install/Install';
import { Keybindings } from '@/components/Keybindings/Keybindings';
import { Navbar } from '@/components/Navbar/Navbar';
import { Security } from '@/components/Security/Security';
import { page } from '@/components/shared/shared.css';

export default function App() {
  return (
    <div className={page}>
      <Navbar />
      <main id="top">
        <Hero />
        <Features />
        <Keybindings />
        <Security />
        <Install />
      </main>
      <Footer />
    </div>
  );
}
