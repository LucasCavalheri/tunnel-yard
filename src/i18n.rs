//! en / pt-BR catalogs. Keys must stay in parity.
//!
//! Copy rules: plain words people already use ("Connect", not "Bring up"), full sentences
//! in hints, no em dashes, and pt-BR written the way a Brazilian says it, not translated
//! word by word. Tech terms Brazilians keep in English (gateway, DNS, host, realm) stay.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

pub type AppLocale = &'static str;
pub type MessageKey = &'static str;

const MESSAGES: &[(&str, &str, &str)] = &[
    // boot
    ("boot.sequence", "Starting TunnelYard…", "Abrindo o TunnelYard…"),
    ("boot.fault", "TunnelYard could not start", "O TunnelYard não conseguiu abrir"),
    ("boot.retry", "Try again", "Tentar de novo"),
    (
        "boot.probeFailed",
        "Could not check the VPN client (openfortivpn).",
        "Não foi possível verificar o cliente de VPN (openfortivpn).",
    ),
    ("brand.subtitle", "FortiGate VPN for Linux", "VPN FortiGate no Linux"),
    // statuses
    ("status.linkUp", "Connected", "Conectado"),
    ("status.handshake", "Connecting", "Conectando"),
    ("status.fault", "Error", "Erro"),
    ("status.idle", "Disconnected", "Desconectado"),
    // sidebar and preferences
    ("ops.autoRelink", "Reconnect automatically", "Reconectar automaticamente"),
    (
        "ops.autoRelinkHint",
        "If a tunnel drops without you asking, TunnelYard connects it again. Disconnecting by hand never triggers this.",
        "Se um túnel cair sem você pedir, o TunnelYard conecta de novo. Desconectar manualmente não dispara isso.",
    ),
    ("ops.reloadProfiles", "Reload profiles", "Recarregar perfis"),
    ("ops.killAll", "Disconnect all", "Desconectar todos"),
    ("ops.startWithLinux", "Open at login", "Abrir ao entrar no sistema"),
    (
        "ops.startWithLinuxHint",
        "TunnelYard starts hidden in the system tray when you log in.",
        "O TunnelYard abre escondido na bandeja quando você entra no sistema.",
    ),
    ("ops.language", "Language", "Idioma"),
    ("ops.languageHint", "Used everywhere in the app.", "Usado em todo o app."),
    ("ops.newProfile", "New profile", "Novo perfil"),
    ("ops.importConf", "Import .conf", "Importar .conf"),
    ("ops.quickActions", "Actions", "Ações"),
    ("ops.workspace", "VPN PROFILES", "PERFIS DE VPN"),
    (
        "ops.workspaceSummary",
        "{total} profiles · {active} connected",
        "{total} perfis · {active} conectados",
    ),
    ("ops.totalProfiles", "profiles", "perfis"),
    ("ops.connectedNow", "connected", "conectados"),
    ("ops.connectingNow", "connecting", "conectando"),
    ("ops.preferences", "Preferences", "Preferências"),
    (
        "ops.preferencesHint",
        "Language, appearance, connections and maintenance.",
        "Idioma, aparência, conexões e manutenção.",
    ),
    ("ops.connectionSettings", "Connections", "Conexões"),
    (
        "ops.connectionSettingsHint",
        "What happens when you log in and when a tunnel drops.",
        "O que acontece quando você entra no sistema e quando um túnel cai.",
    ),
    ("ops.maintenance", "Maintenance", "Manutenção"),
    ("ops.protected", "Runs without root", "Roda sem root"),
    (
        "ops.unprivileged",
        "PolicyKit asks for permission only when a tunnel starts.",
        "O PolicyKit só pede permissão na hora de conectar.",
    ),
    ("ops.connectionOne", "{count} tunnel connected", "{count} túnel conectado"),
    ("ops.connectionMany", "{count} tunnels connected", "{count} túneis conectados"),
    ("ops.connectionsStable", "Everything is running", "Tudo funcionando"),
    ("ops.noneActive", "Connect a profile to start", "Conecte um perfil para começar"),
    (
        "ops.deskSummary",
        "{up} connected · {connecting} connecting",
        "{up} conectados · {connecting} conectando",
    ),
    ("ops.noneConnected", "No tunnel connected", "Nenhum túnel conectado"),
    ("ops.quit", "Quit", "Sair"),
    ("ops.quitConfirm", "Quit TunnelYard?", "Sair do TunnelYard?"),
    (
        "ops.quitConfirmBody",
        "Every connected tunnel will be disconnected.",
        "Todos os túneis conectados serão desconectados.",
    ),
    ("ops.hideToTray", "Hide to tray", "Esconder na bandeja"),
    ("ops.search", "Search profiles", "Buscar perfis"),
    ("ops.tunnels", "Your tunnels", "Seus túneis"),
    // appearance
    ("theme.appearance", "Appearance", "Aparência"),
    (
        "theme.appearanceHint",
        "Light, dark or the same as your system.",
        "Claro, escuro ou igual ao sistema.",
    ),
    ("theme.shortSystem", "System", "Sistema"),
    ("theme.shortLight", "Light", "Claro"),
    ("theme.shortDark", "Dark", "Escuro"),
    ("theme.autoHint", "Follows your desktop", "Acompanha o sistema"),
    ("theme.lightHint", "Always light", "Sempre claro"),
    ("theme.darkHint", "Always dark", "Sempre escuro"),
    // notifications
    ("notify.parkedTitle", "TunnelYard is still running", "O TunnelYard continua aberto"),
    (
        "notify.parkedBody",
        "Your tunnels stay connected. Click the tray icon to open the window again.",
        "Seus túneis continuam conectados. Clique no ícone da bandeja para abrir a janela de novo.",
    ),
    ("notify.connectedTitle", "VPN connected", "VPN conectada"),
    ("notify.connectedBody", "{id} is connected.", "A VPN {id} está conectada."),
    ("notify.disconnectedTitle", "VPN disconnected", "VPN desconectada"),
    ("notify.disconnectedBody", "{id} was disconnected.", "A VPN {id} foi desconectada."),
    ("notify.updateTitle", "TunnelYard update available", "Atualização do TunnelYard disponível"),
    (
        "notify.updateBody",
        "Version {latest} is out. You have {current}.",
        "A versão {latest} saiu. Você está na {current}.",
    ),
    // profiles
    ("profiles.noMatchTitle", "No profiles found", "Nenhum perfil encontrado"),
    ("profiles.noMatch", "No profile matches “{query}”.", "Nenhum perfil bate com “{query}”."),
    ("profiles.clearSearch", "Clear search", "Limpar busca"),
    ("profiles.emptyTitle", "No profiles yet", "Nenhum perfil ainda"),
    (
        "profiles.emptyBody",
        "Create a profile, or import a .conf file you already use with openfortivpn.",
        "Crie um perfil ou importe um arquivo .conf que você já usa com o openfortivpn.",
    ),
    ("profiles.live", "Connected · {uptime}", "Conectado · {uptime}"),
    ("profiles.handshake", "Connecting…", "Conectando…"),
    ("profiles.bringUp", "Connect", "Conectar"),
    ("profiles.killLink", "Disconnect", "Desconectar"),
    ("profiles.noUser", "No username", "Sem usuário"),
    ("profiles.edit", "Edit", "Editar"),
    ("profiles.delete", "Delete", "Excluir"),
    (
        "profiles.deleteConfirm",
        "Delete the profile “{id}”? Its .conf file will be deleted too.",
        "Excluir o perfil “{id}”? O arquivo .conf dele também será apagado.",
    ),
    // console
    ("console.liveTitle", "Console", "Console"),
    ("console.show", "Show console", "Mostrar console"),
    ("console.hide", "Hide console", "Esconder console"),
    ("console.clear", "Clear", "Limpar"),
    (
        "console.emptyCompact",
        "Tunnel activity shows up here.",
        "A atividade dos túneis aparece aqui.",
    ),
    // setup gate
    ("setup.title", "Install the VPN client", "Instale o cliente de VPN"),
    ("setup.missing", "openfortivpn is not installed", "O openfortivpn não está instalado"),
    (
        "setup.needsClient",
        "TunnelYard uses {engine} to connect to FortiGate SSL VPNs. Detected system:",
        "O TunnelYard usa o {engine} para conectar em VPNs FortiGate SSL. Sistema detectado:",
    ),
    ("setup.installPlan", "Install command · {family}", "Comando de instalação · {family}"),
    (
        "setup.noAutoInstall",
        "This distro has no automatic install. Install openfortivpn with your package manager, then check again.",
        "Esta distro não tem instalação automática. Instale o openfortivpn pelo gerenciador de pacotes e verifique de novo.",
    ),
    ("setup.installNow", "Install now", "Instalar agora"),
    ("setup.working", "Installing…", "Instalando…"),
    ("setup.recheck", "I installed it, check again", "Já instalei, verificar de novo"),
    (
        "setup.stillMissing",
        "openfortivpn is still missing, or it does not run.",
        "O openfortivpn ainda não foi encontrado, ou não roda.",
    ),
    ("setup.installFailed", "Could not install openfortivpn.", "Não foi possível instalar o openfortivpn."),
    ("setup.alreadyInstalled", "openfortivpn is already installed.", "O openfortivpn já está instalado."),
    ("setup.logDistro", "Detected system: {distro}", "Sistema detectado: {distro}"),
    ("setup.logFamily", "Package manager: {family}", "Gerenciador de pacotes: {family}"),
    ("setup.logCommand", "Command: {command}", "Comando: {command}"),
    (
        "setup.authCancelled",
        "The permission request was cancelled, or pkexec is not available.",
        "O pedido de permissão foi cancelado, ou o pkexec não está disponível.",
    ),
    ("setup.done", "openfortivpn is installed.", "O openfortivpn foi instalado."),
    // tray
    ("tray.show", "Open TunnelYard", "Abrir o TunnelYard"),
    ("tray.disconnectAll", "Disconnect all", "Desconectar todos"),
    ("tray.refresh", "Reload profiles", "Recarregar perfis"),
    ("tray.checkUpdates", "Check for updates", "Procurar atualizações"),
    ("tray.quit", "Quit", "Sair"),
    // profile editor
    ("form.createTitle", "New profile", "Novo perfil"),
    ("form.editTitle", "Edit profile", "Editar perfil"),
    ("form.importTitle", "Import profile", "Importar perfil"),
    (
        "form.editorHint",
        "Saved as an openfortivpn .conf file in /etc/openfortivpn.",
        "Salvo como arquivo .conf do openfortivpn em /etc/openfortivpn.",
    ),
    ("form.sectionConn", "Gateway", "Gateway"),
    ("form.sectionAuth", "Sign-in", "Login"),
    ("form.sectionOpts", "Options", "Opções"),
    ("form.id", "Profile ID", "ID do perfil"),
    (
        "form.idHint",
        "Becomes the file name: a-z, 0-9, - and _",
        "Vira o nome do arquivo: a-z, 0-9, - e _",
    ),
    ("form.host", "Host", "Host"),
    ("form.port", "Port", "Porta"),
    ("form.username", "Username", "Usuário"),
    ("form.password", "Password", "Senha"),
    ("form.passwordHint", "Saved in the .conf file.", "Fica salva no arquivo .conf."),
    ("form.trustedCert", "Trusted certificate", "Certificado confiável"),
    (
        "form.trustedCertHint",
        "SHA256 fingerprint of the gateway certificate, as openfortivpn prints it.",
        "Impressão digital SHA256 do certificado do gateway, como o openfortivpn mostra.",
    ),
    ("form.realm", "Realm", "Realm"),
    ("form.optional", "Optional", "Opcional"),
    (
        "form.extraOptions",
        "{count} extra options from the imported file are kept. Unsupported ones are reported before connecting.",
        "{count} opções extras do arquivo importado foram mantidas. As não suportadas são avisadas antes de conectar.",
    ),
    ("form.persistent", "Retry interval (seconds)", "Intervalo de reconexão (segundos)"),
    (
        "form.persistentHint",
        "openfortivpn's own reconnect interval. 0 turns it off.",
        "Intervalo de reconexão do próprio openfortivpn. 0 desliga.",
    ),
    ("form.healthHost", "Internal service IPv4", "IPv4 de um serviço interno"),
    ("form.healthPort", "Internal service port", "Porta do serviço interno"),
    ("form.noDtls", "Disable DTLS", "Desativar DTLS"),
    ("form.legacyTunnel", "Legacy FortiGate tunnel", "Túnel FortiGate legado"),
    ("form.setDns", "Use the VPN's DNS", "Usar o DNS da VPN"),
    ("form.setRoutes", "Add the VPN's routes", "Adicionar as rotas da VPN"),
    ("form.save", "Save profile", "Salvar perfil"),
    ("form.cancel", "Cancel", "Cancelar"),
    ("form.close", "Close", "Fechar"),
    ("form.done", "Done", "Pronto"),
    // profile files (conf.rs)
    (
        "conf.badLine",
        "Invalid line in the .conf file (expected key = value).",
        "Linha inválida no arquivo .conf (o formato é chave = valor).",
    ),
    ("conf.multiline", "Profile fields must fit on one line.", "Os campos do perfil precisam caber em uma linha."),
    ("conf.badExtra", "Invalid extra option in the profile.", "Opção extra inválida no perfil."),
    ("conf.duplicate", "This option appears twice in the profile: {key}", "Esta opção aparece duas vezes no perfil: {key}"),
    ("conf.badHealth", "Invalid internal service address.", "Endereço do serviço interno inválido."),
    (
        "conf.healthNeedsBoth",
        "The internal service needs an IPv4 address and a TCP port (1 to 65535).",
        "O serviço interno precisa de um IPv4 e de uma porta TCP (1 a 65535).",
    ),
    (
        "conf.badId",
        "Invalid profile ID. Start with a lowercase letter or a number, then use a-z, 0-9, - or _.",
        "ID de perfil inválido. Comece com letra minúscula ou número e use a-z, 0-9, - ou _.",
    ),
    ("conf.hostRequired", "The host is required.", "O host é obrigatório."),
    ("conf.exists", "A profile “{id}” already exists.", "Já existe um perfil “{id}”."),
    ("conf.authCancelled", "The permission request was cancelled.", "O pedido de permissão foi cancelado."),
    ("conf.writeFailed", "Could not save {path}", "Não foi possível salvar {path}"),
    ("conf.saved", "Saved {path}", "{path} salvo"),
    (
        "conf.savedUnreadable",
        "Saved, but the profile could not be read back.",
        "Salvo, mas não foi possível ler o perfil de volta.",
    ),
    ("conf.notFound", "Profile file not found.", "Arquivo do perfil não encontrado."),
    ("conf.deleteFailed", "Could not delete {path}", "Não foi possível excluir {path}"),
    ("conf.deleted", "Deleted {path}", "{path} excluído"),
    (
        "conf.parseFailed",
        "Could not read this .conf file. Is the host missing?",
        "Não foi possível ler este arquivo .conf. Falta o host?",
    ),
    // tunnels (vpn.rs)
    ("vpn.connectingTo", "Connecting to {name}…", "Conectando a {name}…"),
    ("vpn.tunnelUp", "Tunnel connected", "Túnel conectado"),
    ("vpn.disconnecting", "Disconnecting…", "Desconectando…"),
    ("vpn.authCancelled", "The permission request was cancelled", "O pedido de permissão foi cancelado"),
    ("vpn.closedCode", "Connection closed (code {code})", "Conexão encerrada (código {code})"),
    ("vpn.unknownCode", "unknown", "desconhecido"),
    ("log.profileMissing", "✗ Profile “{id}” not found", "✗ Perfil “{id}” não encontrado"),
    ("log.alreadyUp", "→ {name} is already connected", "→ {name} já está conectado"),
    ("log.connecting", "→ Connecting to {name} ({host}:{port})", "→ Conectando a {name} ({host}:{port})"),
    ("log.config", "→ Profile file: {path}", "→ Arquivo do perfil: {path}"),
    ("log.policykit", "→ Asking PolicyKit for permission ({id})", "→ Pedindo permissão ao PolicyKit ({id})"),
    ("log.startFailed", "✗ [{id}] Could not start: {error}", "✗ [{id}] Não foi possível iniciar: {error}"),
    ("log.exited", "← [{id}] openfortivpn exited (code {code})", "← [{id}] O openfortivpn encerrou (código {code})"),
    ("log.reconnectIn", "↻ [{id}] Reconnecting in {seconds}s…", "↻ [{id}] Reconectando em {seconds}s…"),
    (
        "log.reconnectSkipped",
        "↻ [{id}] Not reconnecting: it was disconnected by hand or is still connected",
        "↻ [{id}] Sem reconexão: foi desconectado manualmente ou ainda está conectado",
    ),
    ("log.disconnecting", "→ [{id}] Disconnecting…", "→ [{id}] Desconectando…"),
    ("log.disconnected", "← [{id}] Disconnected", "← [{id}] Desconectado"),
    // updates
    (
        "update.available",
        "Version {latest} is available. You have {current}.",
        "A versão {latest} está disponível. Você está na {current}.",
    ),
    ("update.open", "Release notes", "Novidades"),
    ("update.install", "Download and install", "Baixar e instalar"),
    ("update.installing", "Downloading the update…", "Baixando a atualização…"),
    ("update.installFailed", "Could not install the update.", "Não foi possível instalar a atualização."),
    (
        "update.elevationDeclined",
        "Installing needs administrator permission, and it was not given.",
        "A instalação precisa de permissão de administrador, e ela não foi concedida.",
    ),
    ("update.installTitle", "Install the update?", "Instalar a atualização?"),
    (
        "update.installMessage",
        "TunnelYard {latest} will be downloaded and installed. Connected tunnels are disconnected first, and the app restarts when it is done.",
        "O TunnelYard {latest} será baixado e instalado. Os túneis conectados são desconectados antes, e o app reinicia no fim.",
    ),
    ("update.cancel", "Cancel", "Cancelar"),
    ("update.dismiss", "Not now", "Agora não"),
    ("update.checkNow", "Check for updates", "Procurar atualizações"),
    ("update.checking", "Checking…", "Procurando…"),
    (
        "update.upToDate",
        "You have the latest version ({version}).",
        "Você está na versão mais recente ({version}).",
    ),
    (
        "update.checkFailed",
        "Could not check for updates. TunnelYard will try again later.",
        "Não foi possível procurar atualizações. O TunnelYard tenta de novo mais tarde.",
    ),
    (
        "update.noArtifact",
        "This release has no package for this system.",
        "Esta versão não tem pacote para este sistema.",
    ),
    ("update.readFailed", "Could not read the update file: {error}", "Não foi possível ler o arquivo da atualização: {error}"),
    (
        "update.checksum",
        "The download does not match its checksum, so it was not installed.",
        "O download não bate com o checksum, então não foi instalado.",
    ),
    ("update.downloadFailed", "The download failed ({error}).", "O download falhou ({error})."),
    ("update.saveFailed", "Could not save the update: {error}", "Não foi possível salvar a atualização: {error}"),
    ("update.startFailed", "Could not start the installer: {error}", "Não foi possível abrir o instalador: {error}"),
    ("update.installerExit", "The installer stopped with status {code}.", "O instalador parou com o status {code}."),
    (
        "update.noBinary",
        "The Linux archive has no TunnelYard binary.",
        "O arquivo Linux não tem o executável do TunnelYard.",
    ),
    ("update.relaunchFailed", "Could not schedule the restart: {error}", "Não foi possível agendar a reinicialização: {error}"),
    (
        "update.notPackage",
        "This TunnelYard was not installed from a Linux package.",
        "Este TunnelYard não foi instalado por um pacote Linux.",
    ),
    ("update.locateFailed", "Could not find this TunnelYard binary: {error}", "Não foi possível achar o executável do TunnelYard: {error}"),
];

/// The UI language, shared with the backend so its status and console lines match it.
static CURRENT_PT: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Set once at startup and again whenever the person switches language.
pub fn set_current_locale(locale: &str) {
    CURRENT_PT.store(locale == "pt-BR", Ordering::Relaxed);
}

pub fn current_locale() -> AppLocale {
    let pt = OVERRIDE
        .with(|o| o.get())
        .unwrap_or_else(|| CURRENT_PT.load(Ordering::Relaxed));
    if pt {
        "pt-BR"
    } else {
        "en"
    }
}

/// Run `f` with a locale for this thread only, so tests never race the global.
pub fn with_locale<R>(locale: &str, f: impl FnOnce() -> R) -> R {
    let prev = OVERRIDE.with(|o| o.replace(Some(locale == "pt-BR")));
    let out = f();
    OVERRIDE.with(|o| o.set(prev));
    out
}

/// Translate in the current UI language. For backend messages that reach the screen.
pub fn tr(key: &str, vars: &[(&str, String)]) -> String {
    translate(current_locale(), key, vars)
}

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
    detect_locale_from(&locale_env())
}

fn locale_env() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    String::new()
}

/// Portuguese UI only for Brazil and Portugal. Anywhere else starts in English.
pub fn detect_locale_from(raw: &str) -> &'static str {
    for token in raw.split([':', ' ']) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let normalized = token.to_ascii_lowercase();
        let base = normalized.split('.').next().unwrap_or("");
        let mut parts = base.split(['_', '-']);
        let lang = parts.next().unwrap_or("");
        let region = parts.next().unwrap_or("");
        if lang == "c" || lang == "posix" {
            continue;
        }
        if lang == "pt" {
            return if matches!(region, "" | "br" | "pt") {
                "pt-BR"
            } else {
                "en"
            };
        }
        if !lang.is_empty() {
            return "en";
        }
    }
    "en"
}
