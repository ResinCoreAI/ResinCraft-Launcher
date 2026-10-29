# ResinCraft Launcher: recursos

Um launcher de Minecraft moderno e amigável feito pela **Puirkle**, para Windows, macOS e Linux. Sem telemetria e sem conta ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

A versão entre parênteses é aquela em que cada recurso chegou. As mais recentes são a 1.4.0 e a 1.4.1.

## Jogando

- **Instâncias (0.1.0):** instalações separadas para cada versão ou modpack, cada uma com seus próprios mods, mundos, Java, memória e argumentos da JVM.
- **Qualquer versão do Minecraft (0.1.0):** todos os lançamentos e snapshots, até as alphas clássicas.
- **Mod loaders (0.1.0):** Fabric, Quilt, Forge e NeoForge, com seletor de versão do loader. Forge clássico para 1.7.10 a 1.12.2 (1.1.2), Forge até o Minecraft 1.2.5 e LiteLoader (1.2.0). OptiFine (1.0.1) e jar mods (1.2.0) também.
- **Mudar versão e loader (1.2.7):** troque a versão do Minecraft ou o mod loader de uma instância na aba Configurações dela, como no Prism. A etiqueta de versão na página da instância abre essa tela.
- **Jogar (0.1.0):** um clique. **Jogar também como** outra conta, ou rode a mesma instância duas vezes com duas contas (1.0.5).
- **Entrada rápida (1.1.0):** seus servidores recentes, e mundos de um jogador (1.2.3), um clique para entrar direto.
- **Mundos (0.1.0):** renomeie um mundo, faça backup, importe um mundo de um zip. Jogue um mundo direto (1.2.0).
- **Backups automáticos dos mundos (1.4.0):** enquanto um jogo ou servidor está rodando, os mundos dele ganham backup a cada 30 minutos, guardando os 3 mais recentes de cada mundo (Configurações → Backup automático).
- **Restaurar um backup (1.4.0):** volte um mundo a partir de qualquer backup, seu ou automático, ou traga-o de volta como uma cópia. O mundo substituído vai para a Lixeira.
- **Corretor de travamentos (1.2.7):** quando o Minecraft trava, o ResinCraft lê o relatório de travamento e os logs. Ele explica em palavras simples o que deu errado e oferece uma correção com um clique. Correções: instalar um mod que está faltando, desativar o mod que travou, escolher o Java certo, dar mais memória ao jogo.
- **Reinstalar (1.3.0):** baixe de novo os arquivos do jogo de uma instância, ou devolva os arquivos de um modpack do jeito que o pack os tem. Pelo menu da instância ou em **Configurações → Reparar**. Os mundos ficam.
- **Flags do Java ajustadas (1.2.7):** os jogos iniciam com as flags G1GC do ResinCraft, para quadros mais suaves (Configurações → Java, ou por instância).
- **Java (0.1.0):** encontra os Javas instalados, escolhe o certo para cada versão do Minecraft e baixa o Eclipse Temurin quando nenhum serve.
- **Uso de RAM (1.2.1):** quanta memória o launcher, cada jogo e cada servidor usam (opcionalmente no Início).
- **Discord Rich Presence (1.2.3):** nome do pack, versão, tempo de jogo, um jogador ou servidor. Botões para os amigos (1.2.6): **View modpack** (ver modpack) e **Get ResinCraft** (baixar o ResinCraft).

## Instâncias

- **Organize:** grupos (0.2.0), sua própria ordem (0.3.1, arraste para organizar), ícones e banners (0.3.0) e cores (0.1.0).
- **Abas (0.1.0):** Conteúdo, Mundos, Servidores, Capturas, Notas, Arquivos (gerenciador e editor de arquivos embutido, 0.2.0), Logs e Configurações.
- **Copiar de outra instância (1.2.5):** mundos, shaders, pacotes de recursos e configurações do jogo; mods e as configurações deles (pasta config) desde a 1.2.6.
- **Duplicar (0.1.0), exportar** (Modrinth `.mrpack` ou CurseForge `.zip`, 1.0.5) e **transformar em seu próprio modpack (1.1.0)**.
- **Selecionar várias (1.1.0):** mova, exclua, duplique ou atualize muitas instâncias de uma vez.
- **Comandos e ambiente (1.2.0):** comandos para rodar antes de iniciar e depois que o jogo fecha, e variáveis de ambiente.
- **Interruptores instantâneos (1.2.7):** ativar ou desativar um mod é instantâneo; a renomeação acontece em segundo plano.
- **Excluir e criar na hora (1.3.0):** uma instância excluída some na hora e vai para a Lixeira em segundo plano. Um modpack sendo instalado aparece na Biblioteca imediatamente, com o progresso no card.

## Conteúdo e Descobrir

- **Sites (0.1.0):** mods, modpacks, pacotes de recursos e shaders do Modrinth, CurseForge, FTB, ATLauncher, Technic e releases do GitHub (0.3.1), filtrados pela versão e pelo loader da instância.
- **Páginas de modpack (1.2.0)** com descrição, galeria, versões, changelogs e **os mods que o pack contém (1.2.5)**.
- **Instalações inteligentes (0.3.0):** escolha qualquer versão, instale vários de uma vez, dependências obrigatórias adicionadas automaticamente, e o mesmo mod vindo de dois sites é detectado.
- **Atualizações (0.1.0):** procure e aplique atualizações (uma ou todas); atualizações de modpack e troca de versão (1.1.0). **Atualizar tudo** baixa até quatro de cada vez (1.4.1).
- **Downloads manuais (1.0.1):** arquivos do CurseForge cujos autores bloqueiam launchers são encontrados na sua pasta Downloads ou em outras **Pastas observadas** (1.0.5, Configurações → Downloads).
- **Listas (0.4.0):** exporte os mods, pacotes de recursos ou shaders como uma lista e instale uma lista em outra instância.
- **Encontrar servidores (1.4.0):** Descobrir → **Servidores** lista servidores hospedados com o ResinCraft e servidores que os jogadores adicionaram. Pesquise e filtre por categoria, região, idioma, modpack, suporte a Bedrock ou a perfis offline; curta e denuncie.
- **Uma página para cada servidor (1.4.1):** a descrição completa e o MOTD, todos os detalhes, o pack do cliente, links e um campo para pedir whitelist.
- **Entrar com um clique (1.4.0):** **Entrar** escolhe a instância que combina, ou pergunta. **Instalar e entrar** cria uma instância com exatamente os mods do servidor e entra.
- **Pedir para entrar na whitelist (1.4.0):** **Pedir para entrar** envia um pedido assinado com a sua conta do Minecraft. Ele mostra **Waiting for the host** (aguardando o host), **On the whitelist** (na whitelist) ou **Ask again** (pedir de novo), e o ResinCraft te avisa (e pisca a barra de tarefas) quando o host responde (1.4.1).

## Hospedando servidores

- **Tipos de servidor (0.1.0):** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge e NeoForge; Leaf, Arclight (mods e plugins juntos) e Bedrock Dedicated Server desde a 0.3.1.
- **A partir de um modpack ou de uma instância (0.1.0):** crie um servidor a partir do Modrinth, do CurseForge (com o server pack oficial, quando existe) ou do FTB, ou a partir de uma das suas instâncias.
- **Sem server pack oficial? (1.2.11):** o ResinCraft monta o servidor a partir da lista de mods do próprio pack, avisa isso quando o servidor é criado e mostra um selo **Built by ResinCraft** (montado pelo ResinCraft).
- **Arquivos para baixar você mesmo (1.2.9):** arquivos cujos autores só permitem download pelo site deles aparecem na aba **Console** do servidor. Antes de cada início, o ResinCraft procura esses arquivos na sua pasta Downloads e nas suas instâncias do mesmo pack, e os coloca no lugar.
- **Memória pelo tamanho do pack (1.2.9):** um servidor novo com mods recebe 6 GB a partir de 80 mods e 8 GB a partir de 150 (no máximo metade da RAM do seu PC).
- **Mods só de cliente desativados só quando travam (1.2.8):** os mods de cliente continuam no servidor enquanto ele inicia bem. Se um início travar porque um mod tentou usar o cliente do jogo, esse mod é desativado (`.jar.disabled`) e o servidor inicia de novo sozinho. Um mod que outro mod precisa continua ativo.
- **Gerencie (0.1.0):** console, editor de propriedades, gerenciador de plugins e mods com atualizações (0.3.0), whitelist, ops e banimentos, ícone do servidor (1.2.5), backups.
- **Atualizações mais rápidas (1.4.1):** **Atualizar tudo** baixa até quatro plugins ou mods de cada vez e coloca todos juntos, ou deixa na fila para o próximo reinício enquanto o servidor está rodando.
- **Divulgue seu servidor (1.4.0):** Configurações → **Lista de servidores** coloca o servidor em Descobrir → Servidores com descrição, tags, região e idioma. Servidores que você roda em outro lugar podem ser adicionados com **Enviar um servidor**.
- **Packs do cliente (1.4.0):** publique o que os jogadores precisam para entrar. Desde a 1.4.1: o modpack do servidor (instalado do próprio site dele), cada outro mod do Modrinth ou do CurseForge e seus próprios arquivos direto do seu PC; pacotes de recursos e shaders junto; servidores de plugins e vanilla também.
- **Reiniciar e menu "mais" (1.2.7):** Reiniciar ao lado de Parar; um menu com copiar endereço, abrir pasta, fazer backup, configurações e excluir.
- **Online agora (1.2.7):** os jogadores no seu servidor, verificados ao vivo a cada poucos segundos.
- **Flags do Java (0.2.0):** as flags ajustadas do ResinCraft ou as do próprio modpack (`user_jvm_args.txt`, scripts de inicialização, 1.2.7).
- **Anti-xray (1.3.0)** (anti-raio-x): servidores Paper, Purpur, Folia e Leaf podem esconder minérios de mods e pacotes de textura de x-ray (Settings → Anti-xray).
- **Anti-cheat (1.4.0)** (antitrapaça): uma chave instala o anti-cheat Grim em servidores de plugins.
- **Regenerar mundo (1.4.0):** recomece um servidor com um mundo novo; o antigo vai para a Lixeira.
- **Reinstalar (1.3.0):** instale de novo o software de um servidor ou devolva os arquivos do modpack dele. O mundo e as configurações ficam.
- **Excluir e criar na hora (1.3.0):** um servidor excluído some na hora; um servidor sendo criado aparece na lista imediatamente, com o progresso.
- **Controle remoto (1.3.0):** gerencie seus servidores de qualquer navegador ou do celular com o console web em `console.rscraft.resincore.tech`, entrando com a mesma conta do Minecraft, enquanto o ResinCraft estiver aberto ou na bandeja do sistema (Configurações → Controle remoto). Ele faz tudo o que o gerenciador de servidores faz. O gerenciador de arquivos, as configurações do Java e plugins de qualquer link precisam de uma segunda chave, **Permitir controle total**. Um botão **Console web** na página Servers (1.3.1).
- **Seus outros PCs (1.4.1):** o menu de PC da aba Servers abre os servidores dos seus outros PCs com controle remoto ativado: inicie, pare, gerencie jogadores, plugins e configurações, e crie ou importe servidores lá.
- **Deixe amigos ajudarem (1.4.1):** Configurações → Controle remoto → **People who can use your servers** (pessoas que podem usar seus servidores): dê a uma conta do Minecraft acesso de **Viewer** (visualizador), **Operator** (operador) ou **Manager** (gerente), a todos os seus servidores ou aos que você escolher. Arquivos, configurações do Java e criar ou excluir servidores continuam só com você.
- **Compartilhando com amigos:**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - seu roteador (UPnP, 0.2.0)
  - seu próprio domínio, com DNS automatizado no Cloudflare (0.2.0), verificado com um registro TXT (**Verificação pendente**, **Verificado**, 1.3.1)
  - **Automatic** (automático) para servidores novos (1.3.2): primeiro seu roteador, depois o ResinCraft Relay, depois o playit.gg
- **Bedrock players can join (0.3.1)** (jogadores do Bedrock podem entrar) em servidores Java (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (pedidos de whitelist): os jogadores pedem para entrar, e você aceita ou recusa na aba Players.
  - **Página web:** no próprio endereço do seu servidor ou no seu domínio verificado (1.3.2, com o ResinCraft Relay).
  - **Seu próprio bot do Discord:** os jogadores digitam `/whitelist`.
  - **A lista de servidores (1.4.0):** jogadores com o ResinCraft pedem em Descobrir → Servidores.
  - **Sua resposta:** aceitar coloca o jogador na whitelist; o bot avisa por mensagem direta.
  - **Proteção:** jogadores banidos são recusados; **Reject and block** (recusar e bloquear) impede que a pessoa peça de novo, com uma lista **Blocked** (bloqueados) e **Unblock** (desbloquear) para cada jogador (1.4.1).

## Bedrock Edition (Windows)

- **Página do Bedrock (0.3.1):** instale (1.0.0), atualize e repare (1.2.0), abra e desinstale (1.2.3) o Minecraft for Windows e o Minecraft Preview (os mundos ganham um backup antes).

## Contas

- **Login com a Microsoft (0.1.0)**, com tokens guardados criptografados. **Perfis offline (0.1.0)** para um jogador e LAN. **Perfis Bedrock (1.0.0)** para contas Microsoft sem a Java Edition.
- **Skins, capas e troca de nome (0.1.0).** Uma biblioteca de skins, skins pelo nome de um jogador e busca online (1.0.3); um visualizador de skin 3D (1.1.0).

## Vindo de outro launcher

- **Importar (1.0.5):** instâncias do Prism Launcher, MultiMC, do app do Modrinth, do app do CurseForge, ATLauncher, GDLauncher e do Minecraft Launcher oficial, com seus mods, mundos, packs e configurações. Servidores também podem ser importados.
- **Mudar para outro PC (1.1.1):** Configurações → Mudar para outro PC salva tudo em um único arquivo `.resincraft`; abra-o no PC novo.

## O app

- **Windows (0.1.0), macOS** (Apple Silicon e Intel) **e Linux** (x64 e ARM64: AppImage, `.deb`, 1.2.0; `.tar.gz`, 1.2.4).
- **Instaladores para Windows:** o `.exe` (0.1.0), um `.msi` para instalações gerenciadas (1.2.10) e o **ResinCraft Setup (1.3.0)**: um instalador pequeno que sempre instala a versão mais recente, por um link que nunca muda.
- **Atualizações (0.3.0):** verificadas automaticamente.
  - Um botão **Atualizar** na barra de título (1.2.7).
  - **Fechar e atualizar**, quando há jogos ou servidores rodando (1.2.7).
  - **Atualizar em silêncio** em segundo plano (1.2.7).
- **Continuar na bandeja do sistema (1.2.7):** fechar a janela mantém os servidores, o bot do Discord, o controle remoto e os downloads funcionando.
- **Reinstalar o ResinCraft (1.3.0):** Configurações → Atualizações instala o app de novo por cima dele mesmo, para repará-lo. Instâncias, contas e configurações ficam.
- **Temas:** Escuro, Claro e Sistema (0.1.0); Obsidiana, Ardósia, Cerejeira, Nether e O End (1.3.0).
  - **Criar tema (1.3.0):** seu próprio tema, com **Ajuste fino** para qualquer cor (seletores de cor) e CSS para partes do app (1.3.2).
  - **Compartilhar temas (1.3.2):** **Exportar** um arquivo `.rctheme`, com a imagem e o CSS dele; **Importar tema**, ou clique duas vezes no arquivo (1.4.0).
  - **CSS personalizado (1.3.0):** seu próprio CSS sobre qualquer tema, começando pelas cores atuais, com um guia embutido das cores, das partes do app e exemplos prontos.
  - E uma surpresa escondida (1.3.0). Tente o código Konami.
- **16 idiomas (0.1.0):** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Gerenciador de armazenamento (1.1.0):** guarde instâncias em outro disco. **Mover instâncias** para lá, ou **Usar sem mover** (1.4.1).
- **Redefinir configurações** e **Desinstalar o ResinCraft (1.4.0)** em Configurações.
