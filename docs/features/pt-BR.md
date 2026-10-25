# ResinCraft Launcher: recursos

Um launcher de Minecraft moderno e amigável feito pela **Puirkle**, para Windows, macOS e Linux. Sem telemetria e sem conta ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Os itens marcados com **(novo)** são novos na versão mais recente.

## Jogando

- **Instâncias:** instalações separadas para cada versão ou modpack, cada uma com seus próprios mods, mundos, Java, memória e argumentos da JVM.
- **Qualquer versão do Minecraft:** todos os lançamentos e snapshots, até as alphas clássicas.
- **Mod loaders:** Fabric, Quilt, Forge (até o Minecraft 1.2.5), NeoForge e LiteLoader, com seletor de versão do loader. OptiFine e jar mods também.
- **Mudar versão e loader:** troque a versão do Minecraft ou o mod loader de uma instância na aba Configurações dela, como no Prism. A etiqueta de versão na página da instância abre essa tela.
- **Jogar:** um clique. **Jogar também como** outra conta, ou rode a mesma instância duas vezes com duas contas.
- **Entrada rápida:** seus servidores recentes e mundos de um jogador, um clique para entrar direto.
- **Mundos:** jogue um mundo direto, renomeie, faça backup, importe um mundo de um zip.
- **Corretor de travamentos:** quando o Minecraft trava, o ResinCraft lê o relatório de travamento e os logs. Ele explica em palavras simples o que deu errado e oferece uma correção com um clique. Correções: instalar um mod que está faltando, desativar o mod que travou, escolher o Java certo, dar mais memória ao jogo.
- **Reinstalar (novo):** baixe de novo os arquivos do jogo de uma instância, ou devolva os arquivos de um modpack do jeito que o pack os tem. Pelo menu da instância ou em **Configurações → Reparar**. Os mundos ficam.
- **Flags do Java ajustadas:** os jogos iniciam com as flags G1GC do ResinCraft, para quadros mais suaves (Configurações → Java, ou por instância).
- **Java:** encontra os Javas instalados, escolhe o certo para cada versão do Minecraft e baixa o Eclipse Temurin quando nenhum serve.
- **Uso de RAM:** quanta memória o launcher, cada jogo e cada servidor usam (opcionalmente no Início).
- **Discord Rich Presence:** nome do pack, versão, tempo de jogo, um jogador ou servidor. Botões para os amigos: **View modpack** (ver modpack) e **Get ResinCraft** (baixar o ResinCraft).

## Instâncias

- **Organize:** grupos, sua própria ordem (arraste para organizar), ícones, banners e cores.
- **Abas:** Conteúdo, Mundos, Servidores, Capturas, Notas, Arquivos (gerenciador e editor de arquivos embutido), Logs e Configurações.
- **Copiar de outra instância:** mundos, mods e as configurações deles (pasta config), shaders, pacotes de recursos e configurações do jogo.
- **Duplicar, exportar** (Modrinth `.mrpack` ou CurseForge `.zip`) e **transformar em seu próprio modpack**.
- **Comandos e ambiente:** comandos para rodar antes de iniciar e depois que o jogo fecha, e variáveis de ambiente.
- **Interruptores instantâneos:** ativar ou desativar um mod é instantâneo; a renomeação acontece em segundo plano.
- **Excluir e criar na hora (novo):** uma instância excluída some na hora e vai para a Lixeira em segundo plano. Um modpack sendo instalado aparece na Biblioteca imediatamente, com o progresso no card.

## Conteúdo e Descobrir

- **Sites:** mods, modpacks, pacotes de recursos e shaders do Modrinth, CurseForge, FTB, ATLauncher, Technic e releases do GitHub, filtrados pela versão e pelo loader da instância.
- **Páginas de modpack** com descrição, galeria, versões, changelogs e **os mods que o pack contém**.
- **Instalações inteligentes:** escolha qualquer versão, instale vários de uma vez, dependências obrigatórias adicionadas automaticamente, e o mesmo mod vindo de dois sites é detectado.
- **Atualizações:** procure e aplique atualizações (uma ou todas); atualizações de modpack e troca de versão.
- **Downloads manuais:** arquivos do CurseForge cujos autores bloqueiam launchers são encontrados na sua pasta Downloads ou em outras **Pastas observadas** (Configurações → Downloads).
- **Listas:** exporte os mods, pacotes de recursos ou shaders como uma lista e instale uma lista em outra instância.

## Hospedando servidores

- **Tipos de servidor:** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mods e plugins juntos) e Bedrock Dedicated Server.
- **A partir de um modpack ou de uma instância:** crie um servidor a partir do Modrinth, do CurseForge (com o server pack oficial, quando existe) ou do FTB, ou a partir de uma das suas instâncias.
- **Sem server pack oficial? (novo):** o ResinCraft monta o servidor a partir da lista de mods do próprio pack, avisa isso quando o servidor é criado e mostra um selo **Built by ResinCraft** (montado pelo ResinCraft).
- **Arquivos para baixar você mesmo (novo):** arquivos cujos autores só permitem download pelo site deles aparecem na aba **Console** do servidor. Antes de cada início, o ResinCraft procura esses arquivos na sua pasta Downloads e nas suas instâncias do mesmo pack, e os coloca no lugar.
- **Memória pelo tamanho do pack (novo):** um servidor novo com mods recebe 6 GB a partir de 80 mods e 8 GB a partir de 150 (no máximo metade da RAM do seu PC).
- **Mods só de cliente desativados só quando travam:** os mods de cliente continuam no servidor enquanto ele inicia bem. Se um início travar porque um mod tentou usar o cliente do jogo, esse mod é desativado (`.jar.disabled`) e o servidor inicia de novo sozinho. Um mod que outro mod precisa continua ativo.
- **Gerencie:** console, editor de propriedades, gerenciador de plugins e mods com atualizações, whitelist, ops e banimentos, ícone do servidor, backups.
- **Reiniciar e menu "mais":** Reiniciar ao lado de Parar; um menu com copiar endereço, abrir pasta, fazer backup, configurações e excluir.
- **Online agora:** os jogadores no seu servidor, verificados ao vivo a cada poucos segundos.
- **Flags do Java:** as flags ajustadas do ResinCraft ou as do próprio modpack (`user_jvm_args.txt`, scripts de inicialização).
- **Anti-xray (novo)** (anti-raio-x): servidores Paper, Purpur, Folia e Leaf podem esconder minérios de mods e pacotes de textura de x-ray (Settings → Anti-xray).
- **Reinstalar (novo):** instale de novo o software de um servidor ou devolva os arquivos do modpack dele. O mundo e as configurações ficam.
- **Excluir e criar na hora (novo):** um servidor excluído some na hora; um servidor sendo criado aparece na lista imediatamente, com o progresso.
- **Controle remoto (novo):** gerencie seus servidores de qualquer navegador ou do celular com o console web em `console.rscraft.resincore.tech`, entrando com a mesma conta do Minecraft, enquanto o ResinCraft estiver aberto ou na bandeja do sistema (Configurações → Controle remoto). Ele faz tudo o que o gerenciador de servidores faz. O gerenciador de arquivos, as configurações do Java e plugins de qualquer link precisam de uma segunda chave, **Permitir controle total**.
- **Compartilhando com amigos:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - seu roteador (UPnP)
  - seu próprio domínio, com DNS automatizado no Cloudflare
- **Bedrock players can join** (jogadores do Bedrock podem entrar) em servidores Java (Geyser + Floodgate).
- **Whitelist requests** (pedidos de whitelist): os jogadores pedem para entrar, e você aceita ou recusa na aba Players.
  - **Página web:** `https://rscraft.resincore.tech/join/<name>` (com o ResinCraft Relay).
  - **Seu próprio bot do Discord:** os jogadores digitam `/whitelist`.
  - **Sua resposta:** aceitar coloca o jogador na whitelist; o bot avisa por mensagem direta.
  - **Proteção:** jogadores banidos são recusados; **Reject and block** (recusar e bloquear) impede que a pessoa peça de novo.

## Bedrock Edition (Windows)

- Instale, abra e desinstale o Minecraft for Windows e o Minecraft Preview (os mundos ganham um backup antes).

## Contas

- **Login com a Microsoft**, com tokens guardados criptografados. **Perfis offline** para um jogador e LAN.
- **Skins, capas e troca de nome.**

## Vindo de outro launcher

- Importe instâncias do Prism Launcher, MultiMC, do app do Modrinth, do app do CurseForge, ATLauncher, GDLauncher e do Minecraft Launcher oficial, com seus mods, mundos, packs e configurações. Servidores também podem ser importados.

## O app

- **Windows, macOS** (Apple Silicon e Intel) **e Linux** (x64 e ARM64: AppImage, `.deb`, `.tar.gz`).
- **Instaladores para Windows:** o `.exe`, um `.msi` para instalações gerenciadas e o **ResinCraft Setup (novo)**: um instalador pequeno que sempre instala a versão mais recente, por um link que nunca muda.
- **Atualizações:** verificadas automaticamente.
  - Um botão **Atualizar** na barra de título.
  - **Fechar e atualizar**, quando há jogos ou servidores rodando.
  - **Atualizar em silêncio** em segundo plano.
- **Continuar na bandeja do sistema:** fechar a janela mantém os servidores, o bot do Discord, o controle remoto e os downloads funcionando.
- **Reinstalar o ResinCraft (novo):** Configurações → Atualizações instala o app de novo por cima dele mesmo, para repará-lo. Instâncias, contas e configurações ficam.
- **Temas (novo):** Escuro, Claro, Obsidiana, Ardósia, Cerejeira, Nether e O End, ou Sistema.
  - **CSS personalizado:** seu próprio CSS sobre qualquer tema, começando pelas cores atuais, com um guia embutido das cores, das partes do app e exemplos prontos.
  - E uma surpresa escondida. Tente o código Konami.
- **16 idiomas:** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Gerenciador de armazenamento** (mova os dados para outro disco), **backup e restauração** das configurações.
