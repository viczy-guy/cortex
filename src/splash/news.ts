/**
 * What's new in each version (#180), shown once after an update and from
 * Settings → About. Written with each release, in both languages: a text
 * missing in either fails `npm run check`. Nothing downloaded is shown.
 *
 * A release adds its entry on top and drops the oldest beyond `MAX_SHOWN`
 * (src/splash/versions.ts), with its images. An image is a release note's
 * picture squeezed to about 50 KB (a JPEG flattened on `--surface-solid`).
 * A version with no `items` is fixes only: the update screen stays as it
 * was, with a small "What changed" that opens this list.
 */

import type { Language } from "../shared/i18n";
import plans from "./news/v0.8.0-plans.jpg";
import islandPt from "./news/v0.9.0-island-pt.jpg";
import atlantisTab from "./news/v0.9.1-atlantis-tab.jpg";
import reviewsTab from "./news/v0.9.2-reviews-tab.jpg";
import sideOpen from "./news/v0.9.3-side-open.jpg";
import standing from "./news/v0.9.2-standing.jpg";

type Text = Record<Language, string>;

export interface Release {
  version: string;
  /** The release day, as `YYYY-MM-DD`. */
  date: string;
  /** What there is to see, one to four. */
  items: { title: Text; text: Text; image?: string }[];
  fixes: Text[];
}

export const RELEASES: Release[] = [
  {
    version: "0.9.3",
    date: "2026-11-27",
    items: [
      {
        image: sideOpen,
        title: { en: "Opens along the side", pt: "Abre pela lateral" },
        text: {
          en: "At the left or right edge the island opens as on top: it grows along the edge, and the list or the card comes out beside it.",
          pt: "Na esquerda ou na direita, a ilha abre como no topo: cresce ao longo da borda, e a lista ou o card sai ao lado.",
        },
      },
    ],
    fixes: [],
  },
  {
    version: "0.9.2",
    date: "2026-11-27",
    items: [
      {
        image: reviewsTab,
        title: { en: "A new review always speaks", pt: "Revisão nova sempre avisa" },
        text: {
          en: "A new review request rings once and says so, even on an amber or red island; the Reviews tab lists every pull request waiting for you, the new ones on top.",
          pt: "Um pedido de revisão novo toca uma vez e avisa, mesmo com a ilha âmbar ou vermelha; a aba Revisões lista todos os PRs que esperam você, os novos no topo.",
        },
      },
      {
        image: standing,
        title: { en: "Standing at the sides", pt: "De pé nas laterais" },
        text: {
          en: "At the left or right edge the island stands, and a notice's phrase comes in a balloon beside it.",
          pt: "Na esquerda ou na direita a ilha fica de pé, e a frase de um aviso aparece num balão ao lado.",
        },
      },
      {
        title: { en: "Drag it along its edge", pt: "Arraste pela borda" },
        text: {
          en: "Press and drag the island sideways on top, or up and down at the sides; near the middle it snaps back. Settings → Island → Position has Center.",
          pt: "Clique, segure e arraste a ilha para os lados no topo, ou para cima e para baixo nas laterais; perto do meio, ela volta ao centro. Em Ajustes → Ilha → Posição tem Centralizar.",
        },
      },
      {
        title: { en: "What's new", pt: "Novidades" },
        text: {
          en: "This screen: after each update, what the new version brings. To open it again, go to Settings → About → What's new.",
          pt: "Esta tela: depois de cada atualização, o que a versão nova traz. Para reabrir esta tela, vá a Ajustes → Sobre → Novidades.",
        },
      },
    ],
    fixes: [{ en: "Three texts in Settings read more clearly.", pt: "Três textos dos Ajustes ficaram mais claros." }],
  },
  {
    version: "0.9.1",
    date: "2026-11-26",
    items: [
      {
        image: atlantisTab,
        title: { en: "Atlantis, live", pt: "Atlantis ao vivo" },
        text: {
          en: "While Atlantis runs on your pull requests, the island shows its sign beside the count, and a click opens the Atlantis tab with each step.",
          pt: "Com o Atlantis rodando nos seus PRs, a ilha mostra o sinal dele ao lado da contagem, e um clique abre a aba Atlantis com cada etapa.",
        },
      },
    ],
    fixes: [{ en: "The island no longer stutters while it opens or folds.", pt: "A ilha não trava mais ao abrir ou fechar." }],
  },
  {
    version: "0.9.0",
    date: "2026-11-25",
    items: [
      {
        image: islandPt,
        title: { en: "English and Portuguese", pt: "Inglês e português" },
        text: {
          en: "Cortex speaks Portuguese when your system does; Settings → General → Language picks one for good.",
          pt: "O Cortex fala português quando o seu sistema fala; em Ajustes → Geral → Idioma você escolhe de vez.",
        },
      },
    ],
    fixes: [],
  },
  {
    version: "0.8.0",
    date: "2026-11-25",
    items: [
      {
        image: plans,
        title: { en: "Atlantis plans on the island", pt: "Os plans do Atlantis na ilha" },
        text: {
          en: "The island follows each Atlantis plan to its apply, red when a plan destroys something or fails; a line opens the plan's summary.",
          pt: "A ilha acompanha cada plan do Atlantis até o apply, em vermelho quando um plan destrói algo ou falha; uma linha abre o resumo do plan.",
        },
      },
    ],
    fixes: [],
  },
];
