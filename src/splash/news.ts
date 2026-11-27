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
import away from "./news/v0.7.0-away.jpg";
import plans from "./news/v0.8.0-plans.jpg";
import islandPt from "./news/v0.9.0-island-pt.jpg";
import atlantisTab from "./news/v0.9.1-atlantis-tab.jpg";

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
  {
    version: "0.7.0",
    date: "2026-11-25",
    items: [
      {
        image: away,
        title: { en: "While you were away", pt: "Enquanto você estava fora" },
        text: {
          en: "Nothing rings while nobody is looking. Back, the island turns pink with what came meanwhile, until See all or Dismiss.",
          pt: "Nada toca enquanto ninguém está olhando. Na volta, a ilha fica rosa com o que chegou, até Ver tudo ou Dispensar.",
        },
      },
    ],
    fixes: [],
  },
];
