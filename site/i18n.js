// Project Depot temporary website: language (EN / PT-BR).
// Static text in index.html carries data-t="key": English is the markup itself, Portuguese comes from PT.
// Text built by app.js goes through t(key, ...args).
const PT = {
 "nav.project": "Projeto",
 "banner": "Site temporário. O site oficial do Project Depot ficará em <a href=\"https://omsimods.com.br\">omsimods.com.br</a>; endereços e conteúdo aqui podem mudar.",
 "footer": "\n    O Project Depot é um projeto independente da equipe OmsiModsBR, derivado do <a href=\"https://github.com/openOMSI-org/openOMSI\">openOMSI</a> (MIT) e sem relação com os criadores do OMSI 2.\n    O OMSI&nbsp;2 é necessário para jogar. Este é um site temporário. · <a href=\"https://github.com/OmsiModsBR/depot/blob/main/LICENSE\">Licença MIT</a>\n  ",
 "hero.text": "Um runtime independente compatível com o OMSI&nbsp;2, escrito em Rust. O objetivo é a compatibilidade 1:1 com o OMSI&nbsp;2 - uma meta em andamento, ainda não um resultado concluído.",
 "chip.early": "Versão inicial: espere bugs",
 "card.need.h": "Você precisa do OMSI&nbsp;2 original",
 "card.need.p": "O Project Depot <b>não contém conteúdo do jogo</b>. Ele roda com os mapas, veículos e arquivos de uma\n        cópia instalada do OMSI&nbsp;2 e <b>não inicia sem ela</b>.",
 "what.h": "O que é o Project Depot?",
 "what.p": "O Project Depot é um projeto independente da equipe OmsiModsBR, derivado de uma base específica do\n    <a href=\"https://github.com/openOMSI-org/openOMSI\">openOMSI</a>, uma recriação do simulador de ônibus <b>OMSI&nbsp;2</b>\n    em <b>Rust</b>. Ele busca <b>compatibilidade comportamental 1:1 com o OMSI&nbsp;2</b> para mapas, ônibus, cenários,\n    scripts e mods existentes, com uma base estável, poucas regressões e integração seletiva das mudanças do upstream.\n    Isso é uma meta de desenvolvimento, não uma afirmação de que ela já foi alcançada.",
 "c1.h": "Compatibilidade 1:1 como meta",
 "c1.p": "O comportamento é conferido com o OMSI&nbsp;2 caso a caso, com testes, antes de qualquer coisa ser chamada de compatível.",
 "c2.h": "Uma base moderna",
 "c2.p": "64 bits, multithread, com renderizador em wgpu (Metal / Vulkan / DirectX&nbsp;12 herdados da base).",
 "c3.h": "Windows x64 primeiro",
 "c3.p": "A versão 0.2.52 é publicada só para Windows x64. Outras plataformas ainda não são oferecidas como pacotes do Depot.",
 "c4.h": "Upstream seletivo",
 "c4.p": "Mudanças do upstream são revisadas quanto a licença, escopo e regressões antes de entrar - nunca sincronizadas às cegas.",
 "c5.h": "Materiais PBR",
 "c5.p": "Mapas de normal, rugosidade, metalicidade e oclusão ao lado de uma textura dão materiais modernos aos mods. <a href=\"#/docs/PBR\">Saiba mais</a>",
 "c6.h": "Para modders",
 "c6.p": "Formatos de conteúdo e notas de modding estão documentados. <a href=\"#/docs/MODDING\">Saiba mais</a>",
 "c7.h": "Um launcher",
 "c7.p": "Perfis, escalas da tabela de horários, prévia de ônibus e pintura, configurações e uma página de Mods.",
 "c8.h": "Código aberto, MIT",
 "c8.p": "Código e histórico são públicos no GitHub, com a origem e a licença do que vem do upstream registradas.",
 "dl.early.p": "O Project Depot está em fase inicial de desenvolvimento. Coisas podem faltar ou estar quebradas e mudar entre versões. Relate problemas na página de <a href=\"#/issues\">Issues</a>.",
 "dl.req.h": "O OMSI&nbsp;2 é necessário",
 "dl.req.p": "Instale primeiro o OMSI&nbsp;2 original. Na primeira execução, aponte o launcher para a pasta dele (a que contém <code>maps</code> e <code>Vehicles</code>).",
 "dl.loading": "Carregando a versão mais recente…",
 "dl.inst": "Instalação",
 "dl.s1": "<b>Windows (x64):</b> descompacte o zip em uma pasta sua (não em <code>Program Files</code>) e execute <code>depot.exe</code> (ou <code>depot-launcher.exe</code>). O SmartScreen pode avisar sobre um app desconhecido: <i>Mais informações</i> → <i>Executar mesmo assim</i>.",
 "dl.s2": "Na primeira execução, aponte o launcher para a pasta do seu OMSI&nbsp;2. Os mods vão na pasta ao lado do jogo ou pela página <b>Mods</b> do launcher. A pasta original do OMSI&nbsp;2 nunca é alterada.",
 "dl.s3": "O programa não se atualiza sozinho: as novas versões são publicadas em <a href=\"#/releases\">Versões</a> e você as baixa manualmente.",
 "dl.s4": "Outras plataformas não fazem parte da versão atual do Depot.",
 "dl.all": "Todas as versões: <a href=\"#/releases\">Versões</a>. Numeração e política: veja <a href=\"#/docs/VERSIONING\">Versionamento</a>.",
 "nav.overview": "Visão geral",
 "dl.btn": "Baixar",
 "nav.docs": "Documentação",
 "nav.source": "Código-fonte",
 "nav.releases": "Versões",
 "nav.issues": "Issues"
};

const DYN = {
  "loading": ["Loading…", "Carregando…"],
  "chip.loading": ["latest version…", "versão mais recente…"],
  "chip.latest": [v => `Latest: ${v}`, v => `Última: ${v}`],
  "chip.none": ["No release yet", "Nenhuma versão ainda"],
  "dl.none": ["No release has been published yet.", "Nenhuma versão foi publicada ainda."],
  "dl.latest": [v => `Latest version: <b>${v}</b>`, v => `Última versão: <b>${v}</b>`],
  "dl.mine": ["Your system", "Seu sistema"],
  "dl.notin": ["Not in this release.", "Não está nesta versão."],
  "dl.download": ["Download", "Baixar"],
  "plat.windows": ["Windows", "Windows"],
  "plat.windows.note": ["64-bit (x64), Windows 10 or newer", "64 bits (x64), Windows 10 ou mais novo"],
  "doc.notfound": ["Not found", "Não encontrado"],
  "doc.nosuch": ["There is no such document.", "Esse documento não existe."],
  "doc.noload": ["The document could not be loaded.", "Não foi possível carregar o documento."],
  "doc.edit": ["Edit on GitHub", "Editar no GitHub"],
  "doc.enonly": ["This page is available in English only, for now.", "Esta página está disponível apenas em inglês, por enquanto."],
  "api.limit": ["GitHub's hourly request limit is reached - try again later.", "O limite de requisições por hora do GitHub foi atingido - tente de novo mais tarde."],
  "api.status": [s => `GitHub answered ${s}.`, s => `O GitHub respondeu ${s}.`],
  "rel.h": ["Releases", "Versões"],
  "rel.lead": ['Every version, newest first. The newest one is also on the <a href="#/download">Download</a> page.', 'Todas as versões, da mais nova para a mais antiga. A mais nova também está na página <a href="#/download">Download</a>.'],
  "rel.ongh": ["Releases on GitHub", "Versões no GitHub"],
  "rel.none": ["No release has been published yet.", "Nenhuma versão foi publicada ainda."],
  "rel.latest": ["Latest", "Última"],
  "rel.pre": ["Pre-release", "Pré-lançamento"],
  "rel.nonotes": ["No notes.", "Sem notas."],
  "iss.h": ["Issues", "Issues"],
  "iss.lead": ["Bugs and wishes. To report one, you need a GitHub account.", "Bugs e pedidos. Para reportar um, você precisa de uma conta no GitHub."],
  "iss.new": ["New issue", "Nova issue"],
  "iss.open": ["Open", "Abertas"],
  "iss.closed": ["Closed", "Fechadas"],
  "iss.all": ["All", "Todas"],
  "iss.filter": ["Filter by title or label", "Filtrar por título ou rótulo"],
  "iss.ongh": ["Issues on GitHub", "Issues no GitHub"],
  "iss.nothing": ["Nothing here.", "Nada por aqui."],
  "iss.updated": [a => `updated ${a}`, a => `atualizada ${a}`],
  "iss.back": ["All issues", "Todas as issues"],
  "iss.nodesc": ["No description.", "Sem descrição."],
  "iss.stopen": ["Open", "Aberta"],
  "iss.stclosed": ["Closed", "Fechada"],
  "iss.comment": ["Comment on GitHub", "Comentar no GitHub"],
  "ago.now": ["just now", "agora mesmo"],
  "ago.n": [(k, u) => `${k} ${u}${k > 1 ? "s" : ""} ago`, (k, u) => `há ${k} ${({year: "ano", month: k > 1 ? "meses" : "mês", day: "dia", hour: "hora", minute: "minuto"})[u]}${k > 1 && u !== "month" ? "s" : ""}`],
  "doc.USER_GUIDE": ["User guide", "Guia do usuário"],
  "doc.MODDING": ["Modding beyond OMSI 2", "Modding além do OMSI 2"],
  "doc.PBR": ["PBR materials", "Materiais PBR"],
  "doc.BUILDING": ["Building", "Compilação"],
  "doc.FORMATS": ["Content formats", "Formatos de conteúdo"],
  "doc.ARCHITECTURE": ["Architecture", "Arquitetura"],
  "doc.ROUTES": ["Routes", "Rotas"],
  "doc.VERSIONING": ["Versioning & releases", "Versionamento e versões"],
  "aria.lang": ["Language", "Idioma"],
};

const LANGS = ["en", "pt-BR"];
// the saved choice, else the browser's languages in order (Portuguese -> PT-BR), else English
function detectLang() {
  try { const s = localStorage.getItem("lang"); if (LANGS.includes(s)) return s; } catch {}
  for (const l of navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || "en"]) {
    const x = String(l).toLowerCase();
    if (x.startsWith("pt")) return "pt-BR";
    if (x.startsWith("en")) return "en";
  }
  return "en";
}
let lang = detectLang();
function t(key, ...args) {
  const e = DYN[key];
  if (!e) return key;
  const v = e[lang === "pt-BR" ? 1 : 0];
  return typeof v === "function" ? v(...args) : v;
}
// swap the static text of root (the English is kept in data-en the first time)
function applyLang(root) {
  document.documentElement.lang = lang;
  for (const el of (root || document).querySelectorAll("[data-t]")) {
    if (el.dataset.en === undefined) el.dataset.en = el.innerHTML;
    const k = el.dataset.t;
    el.innerHTML = lang === "pt-BR" && PT[k] !== undefined ? PT[k] : el.dataset.en;
  }
  const bar = document.getElementById("lang-btns");
  if (bar) {
    bar.setAttribute("aria-label", t("aria.lang"));
    for (const b of bar.querySelectorAll("button")) b.setAttribute("aria-pressed", String(b.dataset.lang === lang));
  }
}
function setLang(l) {
  if (!LANGS.includes(l) || l === lang) return;
  lang = l;
  try { localStorage.setItem("lang", l); } catch {}
}
