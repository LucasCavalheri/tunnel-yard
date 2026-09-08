# TunnelYard website

Landing page estática construída com Astro.

```bash
npm install
npm run dev
```

Para gerar a versão de produção:

```bash
npm run build
```

`npm run build` roda `astro check` antes de compilar, então erro de tipo — ou
chave de tradução faltando — interrompe o build.

Na Vercel, selecione `site` como **Root Directory**. O framework preset pode
ficar como Astro, o comando de build é `npm run build` e a pasta de saída é
`dist`. O site está publicado em <https://tunnelyard.lucascavalheri.com.br/> e
a Vercel republica a cada push no `master`.

## Idiomas

O site é bilíngue. **Inglês é o padrão** e ocupa a rota `/`; o português fica em
`/pt-br/`. As duas rotas são páginas finas que só escolhem o idioma:

```
src/pages/index.astro        → <Landing locale="en" />
src/pages/pt-br/index.astro  → <Landing locale="pt-BR" />
```

Todo o markup vive em `src/components/Landing.astro`, que recebe o idioma e lê
os textos de `src/i18n/`. Não há string escrita direto no markup — se você
precisar de um texto novo, adicione a chave em `src/i18n/en.ts` primeiro.

`en.ts` é a fonte da verdade: ele exporta o tipo `Dictionary`, e `pt-BR.ts` é
declarado como `Dictionary`. Chave faltando ou com nome errado no português
vira erro de compilação, não string em branco em produção.

Placeholders usam `{nome}` e são preenchidos por `fill()`, que **lança exceção**
se você esquecer de passar um valor — melhor quebrar o build do que renderizar
`{version}` para o visitante. Números passam por `formatNumber()`, que respeita
o idioma (`115.4` em inglês, `115,4` em português).

O seletor de idioma (`LanguageSwitcher.astro`) mostra as bandeiras do Brasil e
dos EUA no header e dentro do menu mobile. O link para cada idioma é rotulado
no próprio idioma de destino, por isso `switcher.toPortuguese` é igual nos dois
dicionários. As bandeiras vêm de `@iconify-json/circle-flags` e são inlined em
build time.

Ao adicionar uma seção, lembre de incluir o link nos **dois** navs (desktop e
mobile) e de registrar o rótulo em `nav`.

## Downloads

O build consulta a última release estável do GitHub e usa os URLs dos assets
publicados. Uma opção ausente ou falha na consulta interrompe o build para evitar
publicar links inventados. Execute um novo deploy após publicar uma release —
uma tag sozinha não basta, o site só vê a versão depois que o job `publish`
termina. Linux oferece x64/ARM64 em DEB, RPM e tar.gz; Windows oferece
x64/ARM64; Intel e Apple Silicon usam o mesmo DMG universal. Os botões da
página levam ao seletor; o botão de cada plataforma baixa o arquivo escolhido
diretamente.

## Memória

`src/data/memory.json` contém 60 amostras reais por cenário e as condições da
coleta. São observações do build debug 2.6.0, não um benchmark controlado.
Não extrapolar para macOS, Windows ou builds release.

A seção do site mostra três coisas e só: o total com dois túneis conectados
(115,4 MiB), a divisão entre interface e motores VPN (75/25) e a variação ao
longo da coleta. **Deliberadamente não há gráfico de série temporal.** A coleta
inteira varia 0,25 MiB — os `engines` são um único valor repetido 60 vezes, e a
série `active` tem 7 valores distintos. Numa escala de 0–320 MiB isso é 0,16
pixel: uma linha reta. Uma versão anterior desta seção tinha um gráfico com
scrubber por segundo, e o resultado foi que ela parecia quebrada.

A medição do app ocioso (241,9 MiB) fica só no "Como medimos", com a ressalva
de que era **outra instância**, com outro tempo de vida e outro estado gráfico.
Ela não vai ao lado do número principal de propósito: lado a lado, os dois
sugerem que conectar reduz o consumo pela metade, o que a coleta não sustenta.

Para repetir no Linux, identifique os PIDs sem expor perfis ou credenciais:

```bash
node scripts/measure-memory.mjs idle=PID active=PID engines=PID,PID
```

Use o mesmo build, estado da janela, período de aquecimento e carga de tráfego
para obter uma comparação controlada. O script apenas lê RSS dos PIDs fornecidos
e imprime JSON; não abre nem encerra conexões. Atualize os dados e a descrição
das condições juntos após uma nova coleta — e se a nova coleta tiver variação
real, aí sim vale reconsiderar um gráfico.
