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

Na Vercel, selecione `site` como **Root Directory**. O framework preset pode
ficar como Astro, o comando de build é `npm run build` e a pasta de saída é
`dist`.

## Downloads

O build consulta a última release estável do GitHub e usa os URLs dos assets
publicados. Uma opção ausente ou falha na consulta interrompe o build para evitar
publicar links inventados. Execute um novo deploy após publicar uma release.
Linux oferece x64/ARM64 em DEB, RPM e tar.gz; Windows oferece x64/ARM64;
Intel e Apple Silicon usam o mesmo DMG universal. Os botões da página levam ao
seletor; o botão de cada plataforma baixa o arquivo escolhido diretamente.

## Memória

`src/data/memory.json` contém 60 amostras reais por cenário e as condições da
coleta. São observações do build debug 2.6.0, não um benchmark controlado da
2.7.0. Não extrapolar para macOS, Windows ou builds release.

Para repetir no Linux, identifique os PIDs sem expor perfis ou credenciais:

```bash
node scripts/measure-memory.mjs idle=PID active=PID engines=PID,PID
```

Use o mesmo build, estado da janela, período de aquecimento e carga de tráfego
para obter uma comparação controlada. O script apenas lê RSS dos PIDs fornecidos
e imprime JSON; não abre nem encerra conexões. Atualize os dados e a descrição
das condições juntos após uma nova coleta.
