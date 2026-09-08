//! en / pt-BR catalogs. Keys must stay in parity.

pub type AppLocale = &'static str;
pub type MessageKey = &'static str;

const MESSAGES: &[(&str, &str, &str)] = &[
    ("boot.sequence", "boot sequence…", "sequência de boot…"),
    ("boot.fault", "Boot fault", "Falha no boot"),
    ("boot.retry", "Retry", "Tentar de novo"),
    ("boot.bridgeMissing", "Could not start TunnelYard. Run `tunnel-yard --smoke` from a terminal to see the error.", "Não foi possível iniciar o TunnelYard. Rode `tunnel-yard --smoke` no terminal para ver o erro."),
    ("brand.subtitle", "OpenForti control desk", "Mesa de controle OpenForti"),
    (
        "brand.subtitleMulti",
        "Manage every environment from one place.",
        "Gerencie todos os ambientes daqui.",
    ),
    ("status.linkUp", "Connected", "Conectado"),
    ("status.handshake", "Connecting", "Conectando"),
    ("status.fault", "Error", "Falha"),
    ("status.idle", "Idle", "Inativo"),
    ("ops.autoRelink", "Auto-relink", "Auto-reconectar"),
    ("ops.reloadProfiles", "Reload profiles", "Recarregar perfis"),
    ("ops.killAll", "Disconnect all", "Desconectar todos"),
    ("ops.parkTray", "Park in tray", "Ir para a bandeja"),
    ("ops.startWithLinux", "Start at login", "Iniciar ao entrar"),
    ("ops.language", "Language", "Idioma"),
    ("ops.desk", "Desk", "Mesa"),
    (
        "ops.noneActive",
        "no active connections",
        "nenhuma conexão ativa",
    ),
    (
        "ops.deskSummary",
        "{up} connected · {handshake} starting",
        "{up} conectadas · {handshake} iniciando",
    ),
    ("ops.newProfile", "New profile", "Novo perfil"),
    ("ops.importConf", "Import .conf", "Importar .conf"),
    ("ops.preferences", "Preferences", "Preferências"),
    (
        "ops.preferencesHint",
        "Connection behavior, language and maintenance.",
        "Comportamento das conexões, idioma e manutenção.",
    ),
    ("ops.connectionSettings", "Connections", "Conexões"),
    ("ops.maintenance", "Maintenance", "Manutenção"),
    ("ops.protected", "Protected execution", "Execução protegida"),
    (
        "ops.unprivileged",
        "Unprivileged interface",
        "Interface sem privilégios",
    ),
    ("ops.connectionOne", "{count} connection", "{count} conexão"),
    ("ops.connectionMany", "{count} connections", "{count} conexões"),
    ("ops.connectionsStable", "all stable", "todas estáveis"),
    ("ops.quit", "Quit", "Sair"),
    ("ops.quitConfirm", "Quit TunnelYard?", "Sair do TunnelYard?"),
    ("ops.quitConfirmBody", "Active tunnels will be disconnected.", "Os túneis ativos serão desconectados."),
    ("ops.hideToTray", "Hide to tray", "Ocultar na bandeja"),
    ("ops.search", "Search profiles", "Buscar perfis"),
    ("ops.tunnels", "Your tunnels", "Seus túneis"),
    ("chrome.closeHides", "Close hides to the tray.", "Fechar oculta na bandeja."),
    ("theme.shortSystem", "Auto", "Auto"),
    ("theme.shortLight", "Light", "Claro"),
    ("theme.shortDark", "Dark", "Escuro"),
    ("notify.parkedTitle", "TunnelYard is still running", "O TunnelYard continua em execução"),
    ("notify.parkedBody", "Tunnels stay up. Open the tray icon to show the window.", "Os túneis continuam ativos. Abra o ícone da bandeja para mostrar a janela."),
    ("profiles.noMatch", "No profiles match this search.", "Nenhum perfil corresponde à busca."),
    ("form.showPassword", "Show", "Mostrar"),
    ("form.hidePassword", "Hide", "Ocultar"),
    ("form.sectionConn", "Gateway", "Gateway"),
    ("form.sectionAuth", "Authentication", "Autenticação"),
    ("form.sectionOpts", "Options", "Opções"),
    ("setup.title", "Set up the VPN client", "Instalar o cliente VPN"),
    ("console.show", "Show console", "Mostrar console"),
    ("console.hide", "Hide", "Ocultar"),
    ("profiles.emptyTitle", "No profiles", "Nenhum perfil"),
    ("profiles.emptyBody", "Create a profile or import an existing openfortivpn .conf file.", "Crie um perfil ou importe um arquivo .conf existente do openfortivpn."),
    ("profiles.live", "live · {uptime}", "ao vivo · {uptime}"),
    ("profiles.handshake", "handshake…", "handshake…"),
    ("profiles.bringUp", "Bring up", "Conectar"),
    ("profiles.killLink", "Disconnect", "Desconectar"),
    ("profiles.noUser", "no-user", "sem-usuário"),
    ("profiles.flagOn", "on", "sim"),
    ("profiles.flagOff", "off", "não"),
    ("profiles.routesShort", "routes", "rotas"),
    ("profiles.edit", "Edit", "Editar"),
    ("profiles.delete", "Delete", "Excluir"),
    ("profiles.deleteConfirm", "Delete profile \"{id}\"?", "Excluir o perfil \"{id}\"?"),
    ("console.title", "Console // {label}", "Console // {label}"),
    ("console.liveTitle", "Live console", "Console ao vivo"),
    ("console.receiving", "receiving", "recebendo"),
    (
        "console.emptyCompact",
        "Waiting for tunnel activity…",
        "Aguardando atividade dos túneis…",
    ),
    ("console.working", " · working", " · trabalhando"),
    ("console.clear", "Clear", "Limpar"),
    ("console.empty", "Waiting for tunnel I/O. You can bring up multiple VPNs at once — each keeps its own link.", "Aguardando I/O do túnel. Dá pra subir várias VPNs ao mesmo tempo — cada uma mantém o próprio link."),
    ("setup.missing", "Missing dependency", "Dependência ausente"),
    ("setup.homebrew", "Install Homebrew from brew.sh first, then run brew install openfortivpn and recheck.", "Instale o Homebrew em brew.sh, execute brew install openfortivpn e verifique novamente."),
    ("setup.needsClient", "TunnelYard needs {engine} to open SSL tunnels. Your operating system:", "O TunnelYard precisa do {engine} para abrir túneis SSL. Seu sistema:"),
    ("setup.looksLike", "{distro}", "{distro}"),
    ("setup.installPlan", "Install plan · {family}", "Plano de instalação · {family}"),
    ("setup.noAutoInstall", "No automatic installer for this distro", "Instalação automática indisponível nesta distro"),
    ("setup.installNow", "Install now", "Instalar agora"),
    ("setup.working", "Working…", "Trabalhando…"),
    ("setup.recheck", "I installed it — recheck", "Já instalei — verificar de novo"),
    ("setup.stillMissing", "The VPN client is still missing or cannot run.", "O cliente VPN ainda está ausente ou não pode ser executado."),
    ("setup.installFailed", "Could not install the VPN client.", "Não foi possível instalar o cliente VPN."),
    ("tray.show", "Show TunnelYard", "Mostrar TunnelYard"),
    ("tray.disconnectAll", "Disconnect all", "Desconectar todas"),
    ("tray.refresh", "Refresh profiles", "Atualizar perfis"),
    ("tray.checkUpdates", "Check for updates", "Verificar atualizações"),
    ("tray.quit", "Quit", "Sair"),
    ("notify.connectedTitle", "VPN connected", "VPN conectada"),
    ("notify.connectedBody", "Tunnel {id} is up.", "Túnel {id} está ativo."),
    ("notify.disconnectedTitle", "VPN disconnected", "VPN desconectada"),
    ("notify.disconnectedBody", "Tunnel {id} ended.", "Túnel {id} encerrou."),
    ("notify.updateTitle", "TunnelYard update available", "Atualização do TunnelYard disponível"),
    ("notify.updateBody", "Version {latest} is out (you have {current}). Install it with one click.", "Saiu a versão {latest} (você tem {current}). Instale com um clique."),
    ("form.createTitle", "New connection", "Nova conexão"),
    ("form.editTitle", "Edit connection", "Editar conexão"),
    ("form.importTitle", "Import connection", "Importar conexão"),
    ("form.id", "Profile id", "ID do perfil"),
    ("form.idHint", "Profile filename (a-z, 0-9, - _)", "Nome do arquivo do perfil (a-z, 0-9, - _)"),
    ("form.host", "Host", "Host"),
    ("form.port", "Port", "Porta"),
    ("form.username", "Username", "Usuário"),
    ("form.password", "Password", "Senha"),
    ("form.passwordHint", "Stored in the .conf (same as your current setup)", "Fica no .conf (igual ao setup atual)"),
    ("form.trustedCert", "Trusted cert", "Trusted cert"),
    ("form.trustedCertHint", "SHA256 fingerprint from openfortivpn / gateway", "Fingerprint SHA256 do openfortivpn / gateway"),
    ("form.realm", "Realm", "Realm"),
    ("form.optional", "Optional", "Opcional"),
    ("form.extraOptions", "{count} additional imported options are preserved. Unsupported options will be reported before connecting.", "{count} opções adicionais importadas serão preservadas. Opções incompatíveis serão informadas antes da conexão."),
    ("form.persistent", "Persistent (sec)", "Persistent (seg)"),
    ("form.persistentHint", "0 = off · reconnect interval used by openfortivpn", "0 = off · intervalo de reconexão do openfortivpn"),
    ("form.healthHost", "Internal service IPv4", "IPv4 do serviço interno"),
    ("form.healthPort", "Service TCP port", "Porta TCP do serviço"),
    ("form.healthHint", "Optional · Windows validates this service through the VPN before showing connected and while the tunnel is active.", "Opcional · no Windows, valida o serviço pela VPN antes de mostrar conectada e enquanto o túnel estiver ativo."),
    ("form.noDtls", "Disable DTLS", "Desativar DTLS"),
    ("form.noDtlsHint", "Use for FortiGate gateways that reject the DTLS hello; stays on HTTPS like openfortivpn.", "Use em gateways FortiGate que rejeitam o hello DTLS; mantém HTTPS como o openfortivpn."),
    ("form.legacyTunnel", "Legacy FortiGate tunnel", "Túnel FortiGate legado"),
    ("form.legacyTunnelHint", "Use when OpenConnect reaches the tunnel but the gateway immediately closes it; matches openfortivpn’s TLS hand-off.", "Use quando o OpenConnect chega ao túnel, mas o gateway fecha logo em seguida; replica a troca TLS do openfortivpn."),
    ("form.setDns", "set-dns", "set-dns"),
    ("form.setRoutes", "set-routes", "set-routes"),
    ("form.save", "Save profile", "Salvar perfil"),
    ("form.saving", "Saving…", "Salvando…"),
    ("form.cancel", "Cancel", "Cancelar"),
    ("form.done", "Done", "Concluir"),
    ("update.available", "Update available · {latest} (you have {current})", "Atualização disponível · {latest} (você tem {current})"),
    ("update.aptHint", "Download and install the package for your operating system. Administrator approval may be requested.", "Baixe e instale o pacote do seu sistema. O sistema pode pedir autorização de administrador."),
    ("update.open", "Release notes", "Notas da release"),
    ("update.install", "Download and install", "Baixar e instalar"),
    ("update.installing", "Downloading update…", "Baixando atualização…"),
    ("update.installFailed", "Could not install this update.", "Não foi possível instalar esta atualização."),
    ("update.elevationDeclined", "The installer needs administrator approval, which was not granted.", "O instalador precisa de autorização de administrador, que não foi concedida."),
    ("update.installTitle", "Install TunnelYard update", "Instalar atualização do TunnelYard"),
    ("update.installMessage", "Version {latest} will be downloaded and installed. Active VPN tunnels will be disconnected first.", "A versão {latest} será baixada e instalada. As VPNs ativas serão desconectadas antes."),
    ("update.cancel", "Cancel", "Cancelar"),
    ("update.cancelled", "Update canceled.", "Atualização cancelada."),
    ("update.dismiss", "Dismiss", "Dispensar"),
    ("update.checkNow", "Check for updates", "Verificar atualizações"),
    ("update.checking", "Checking…", "Verificando…"),
    ("update.upToDate", "You are up to date ({version})", "Você está atualizado ({version})"),
    ("update.checkFailed", "Could not check right now — it will retry.", "Não deu para verificar agora — vou tentar de novo."),
    ("update.noArtifact", "No compatible package was published for this operating system.", "Nenhum pacote compatível foi publicado para este sistema."),
    ("update.restarting", "Restarting TunnelYard…", "Reiniciando o TunnelYard…"),
    ("theme.system", "Follow system theme", "Seguir o tema do sistema"),
    ("theme.light", "Light theme", "Tema claro"),
    ("theme.dark", "Dark theme", "Tema escuro"),
];

fn lookup(locale: &str, key: &str) -> Option<&'static str> {
    MESSAGES.iter().find(|(k, _, _)| *k == key).map(
        |(_, en, pt)| {
            if locale == "pt-BR" {
                *pt
            } else {
                *en
            }
        },
    )
}

pub fn translate(locale: &str, key: &str, vars: &[(&str, String)]) -> String {
    let mut text = lookup(locale, key)
        .or_else(|| lookup("en", key))
        .unwrap_or(key)
        .to_string();
    for (name, value) in vars {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

pub fn list_locales() -> &'static [AppLocale] {
    &["en", "pt-BR"]
}

pub fn catalog_keys(_locale: &str) -> Vec<MessageKey> {
    MESSAGES.iter().map(|(k, _, _)| *k).collect()
}

pub fn detect_locale() -> &'static str {
    let loc = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_lowercase();
    if loc.starts_with("pt") {
        "pt-BR"
    } else {
        "en"
    }
}
