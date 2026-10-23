# ResinCraft Launcher: guia do usuário

Como fazer as coisas mais comuns no ResinCraft. Os nomes em **negrito** são o que você vai ver na tela.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Conteúdo

1. [Instalar e entrar](#1-instalar-e-entrar)
2. [Crie sua primeira instância](#2-crie-sua-primeira-instância)
3. [Adicione mods, pacotes de recursos e shaders](#3-adicione-mods-pacotes-de-recursos-e-shaders)
4. [Instale um modpack](#4-instale-um-modpack)
5. [Mude a versão do Minecraft ou o mod loader](#5-mude-a-versão-do-minecraft-ou-o-mod-loader)
6. [Quando o Minecraft trava](#6-quando-o-minecraft-trava)
7. [Copie coisas de outra instância](#7-copie-coisas-de-outra-instância)
8. [Hospede um servidor](#8-hospede-um-servidor)
9. [Deixe seus amigos entrarem no servidor](#9-deixe-seus-amigos-entrarem-no-servidor)
10. [Pedidos de whitelist (página web e bot do Discord)](#10-pedidos-de-whitelist-página-web-e-bot-do-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Venha de outro launcher](#12-venha-de-outro-launcher)
13. [Atualizações e a bandeja do sistema](#13-atualizações-e-a-bandeja-do-sistema)
14. [Solução de problemas](#14-solução-de-problemas)

## 1. Instalar e entrar

1. Baixe o ResinCraft para o seu sistema na [página de Releases](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` ou `.tar.gz`
2. Instale e abra. O Windows pode dizer que o editor não foi verificado; no macOS, clique com o botão direito no app e escolha **Abrir** na primeira vez.
3. Abra **Contas** → **Entrar com a Microsoft** e entre na própria página da Microsoft. Sua senha nunca passa pelo ResinCraft.
   - Não tem conta Microsoft? Adicione um **Perfil offline** para jogar sozinho e em LAN.

## 2. Crie sua primeira instância

Uma *instância* é uma instalação separada do Minecraft, com seus próprios mods, mundos e configurações.

1. Abra **Biblioteca** → **Nova instância**.
2. Escolha uma **Versão do Minecraft** e um **Mod loader**:
   - **Vanilla:** sem mods.
   - **Fabric** ou **Quilt:** mods leves, mods de desempenho.
   - **Forge** ou **NeoForge:** mods grandes de conteúdo.
3. Dê um nome a ela e clique em **Criar instância**.
4. Clique em **Jogar**. A primeira vez baixa o jogo e o Java certo; nas próximas, abre rapidinho.

Dica: a **Entrada rápida** à esquerda mostra seus servidores e mundos recentes; um clique e você já entra.

## 3. Adicione mods, pacotes de recursos e shaders

1. Abra sua instância → aba **Conteúdo** → **Adicionar mods**. Ou abra **Descobrir** na barra da esquerda.
2. Pesquise ou navegue pelo Modrinth, CurseForge, FTB e os outros sites. Só aparecem as versões que servem para a sua instância.
3. Clique em **Instalar**. Os mods obrigatórios (dependências) são adicionados para você.
4. De volta em **Conteúdo**:
   - use o interruptor ao lado de um mod para ativá-lo ou desativá-lo;
   - clique em **Procurar atualizações** para atualizar os mods (um ou todos).

Alguns autores do CurseForge não deixam launchers baixarem seus arquivos. Nesse caso, o ResinCraft lista esses arquivos em **Baixar tudo**: baixe-os no navegador, e o ResinCraft os encontra sozinho na sua pasta Downloads. Seu navegador salva em outro lugar? Adicione essa pasta em **Configurações → Downloads → Pastas observadas**.

## 4. Instale um modpack

1. Abra **Descobrir** → **Modpacks** (ou **Biblioteca** → **Nova instância** → **Baixar um modpack**).
2. Abra um pack para ver a descrição, as versões e os **Mods** que ele contém.
3. Clique em **Instalar pack**. O ResinCraft cria uma instância nova com tudo dentro.
4. Quando o pack receber uma atualização, o ResinCraft avisa você; você também pode escolher outra versão do pack na página da instância.

## 5. Mude a versão do Minecraft ou o mod loader

1. Na página da instância, clique na etiqueta de versão no topo (por exemplo **1.21.4 · Fabric 0.16.10**). Ela abre **Configurações → Versão**.
2. Escolha a versão do **Minecraft**, o **Carregador de mods** e a versão do loader.
3. Clique em **Mudar versão** e confirme. Na próxima vez que o jogo abrir, ele baixa o que for preciso.
4. Mods feitos para a versão antiga podem não funcionar: vá em **Conteúdo** e clique em **Procurar atualizações**.

## 6. Quando o Minecraft trava

Quando o jogo trava, abre uma janela **(nome da instância) travou**. O ResinCraft lê o relatório de travamento e os logs e diz o que deu errado, por exemplo:

- *Sodium Extra precisa do Sodium, que não está instalado* → **Corrigir** instala o Sodium.
- *citresewn travou o jogo* → **Corrigir** desativa esse mod (depois você pode ativá-lo de novo em **Conteúdo**).
- *Isto precisa do Java 21 ou mais novo* → **Corrigir** deixa o ResinCraft escolher o Java certo.
- *O Minecraft ficou sem memória* → **Corrigir** dá mais memória a ele.

Clique em **Corrigir** em cada um, ou em **Corrigir tudo e jogar**. Nada muda até você clicar em um botão. Se o ResinCraft não descobrir o motivo, **Abrir registros** mostra a última coisa que o jogo disse.

## 7. Copie coisas de outra instância

1. Abra a instância *para* a qual você quer copiar → clique no botão **Copiar de outra instância** no topo.
2. Escolha a instância *de onde* copiar.
3. Marque o que você quer:
   - **Mundos**;
   - **Mods** e **Configurações dos mods** (a pasta config);
   - **Shaders** e **Pacotes de recursos**;
   - **Configurações do jogo** (vídeo, controles, atalhos de teclas).
4. Clique em **Copiar**. Mundos com um nome que já existe ganham um nome novo. Mods e packs que você já tem ficam como estão.

## 8. Hospede um servidor

1. Abra **Servers** → **Create server** (criar servidor).
2. Escolha como:
   - **Server software** (software de servidor): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** o mesmo pack que seus amigos jogam.
   - **From an instance** (de uma instância): os mods que você já usa.
   - **From a file** (de um arquivo).
3. Clique em **Create**. A janela fecha e o download continua no canto inferior direito.
4. Aceite o EULA do Minecraft quando for pedido e clique em **Start**.

Na página de um servidor:

- **Console:** o que o servidor diz; digite comandos embaixo.
- **Players** (jogadores): quem está online agora, além da whitelist, dos operadores e dos banimentos.
- **Mods** ou **Plugins:** adicione, atualize ou desative.
- **Properties** (propriedades): as configurações do servidor (dificuldade, whitelist ligada/desligada e por aí vai).
- **Settings** (configurações):
  - memória, Java e **Java flags** (as do ResinCraft ou as do próprio modpack);
  - como os amigos se conectam;
  - o ícone do servidor.
- **Reiniciar** e o menu **⋯** (copiar endereço, abrir pasta, fazer backup agora, excluir).

Mods que só rodam no cliente do jogo (shaders, minimapas…) são desativados automaticamente nos servidores. Você pode vê-los na aba **Mods** e ativá-los de novo, se precisar.

## 9. Deixe seus amigos entrarem no servidor

Nas **Settings** do servidor → **How friends connect** (como os amigos se conectam):

- **Automatic** (recomendado): tenta primeiro o **ResinCraft Relay**, depois o playit.gg, depois o bore.
- **ResinCraft Relay:** os amigos entram em `yourname.rscraft.resincore.tech`. Precisa de uma conta Microsoft conectada.
- **playit.gg:** um endereço grátis que não muda. Na primeira vez, ele pede para você vincular uma conta playit.gg (uma conta de convidado serve).
- **Porta redirecionada por mim:** se você mesmo configurou o roteador.

A aba **Console** mostra o endereço para passar aos seus amigos. Amigos no Bedrock também podem entrar em servidores Java: ative **Bedrock players can join** (jogadores do Bedrock podem entrar).

## 10. Pedidos de whitelist (página web e bot do Discord)

Deixe os jogadores pedirem para entrar e aceite ou recuse você mesmo.

1. Em **Properties**, defina **white-list** como **true** (senão qualquer um entra de qualquer jeito).
2. Abra a aba **Players** do servidor e ative **Whitelist requests** (pedidos de whitelist).
3. Escolha como os jogadores pedem:
   - **Web page** (página web):
     - Funciona quando o servidor é compartilhado pelo ResinCraft Relay.
     - Clique em **Copy link** (copiar link) e mande o link para os jogadores: `https://rscraft.resincore.tech/join/<name>`.
     - A página funciona enquanto o servidor está online.
   - **Your Discord bot** (seu bot do Discord):
     1. Abra o [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** e copie o token.
     2. Cole no ResinCraft e clique em **Save** (salvar). Ele fica guardado com segurança no seu PC e nunca mais é mostrado.
     3. Quando aparecer **Online as …**, clique em **Add the bot to your Discord** (adicionar o bot ao seu Discord) e escolha seu servidor do Discord.
     4. Os jogadores digitam `/whitelist TheirName` no seu Discord.
4. Os pedidos aparecem na aba **Players**:
   - **Accept** (aceitar): coloca o jogador na whitelist (o bot avisa por mensagem direta).
   - **Reject** (recusar): recusa o pedido.
   - **Reject and block** (recusar e bloquear): recusa e impede que essa pessoa peça de novo. **Unblock all** (desbloquear todos) desfaz os bloqueios do Discord.

Pedidos de jogadores banidos são recusados automaticamente. O bot do Discord funciona enquanto o ResinCraft está aberto; ative a bandeja do sistema (seção 13) para ele continuar funcionando com a janela fechada.

## 11. Bedrock Edition

No Windows, **Bedrock** à esquerda instala, abre e desinstala o Minecraft for Windows e o Minecraft Preview. Antes de desinstalar, ele faz backup dos seus mundos.

## 12. Venha de outro launcher

**Biblioteca** → **Trazer de outro launcher** traz instâncias de:

- Prism Launcher e MultiMC
- app do Modrinth e app do CurseForge
- ATLauncher e GDLauncher
- Minecraft Launcher oficial

Os mods, mundos e configurações vêm junto. Os originais ficam onde estão.

## 13. Atualizações e a bandeja do sistema

- **Atualizações:**
  - Quando uma nova versão do ResinCraft está pronta, aparece um botão **Atualizar** na barra de título.
  - Se um jogo ou servidor estiver rodando, você escolhe **Fechar e atualizar** (os servidores salvam os mundos antes) ou **Atualizar quando pararem**.
- **Atualizar em silêncio** (Configurações → Atualizações): instala versões novas em segundo plano, sem perguntar, quando nada estiver rodando.
- **Continuar na bandeja do sistema** (Configurações → Aparência):
  - Fechar a janela mantém o ResinCraft rodando: servidores, o bot do Discord e os downloads continuam.
  - Clique no ícone da bandeja para abrir de novo; botão direito → **Quit ResinCraft** (sair do ResinCraft) para sair de verdade.

## 14. Solução de problemas

- **O jogo não abre:** veja a janela **(nome da instância) travou** (seção 6) ou a aba **Logs** da instância.
- **Um mod não aparece no Descobrir:** talvez ele não exista para a sua versão do Minecraft ou para o seu loader. Tente outra versão ou mude a versão da instância (seção 5).
- **Os amigos não conseguem entrar:** confira o endereço na aba **Console** do servidor e se o servidor mostra **Running** (rodando). Com o ResinCraft Relay, verifique se você entrou com uma conta Microsoft.
- **O servidor trava ao iniciar:** o ResinCraft desativa sozinho os mods só de cliente e inicia de novo. Procure no **Console** a linha que diz qual mod foi desativado.
- **Ainda com problemas?** Relate o problema no [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) com o log da aba **Logs**.
