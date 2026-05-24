use serde_json::{Value, json};

use crate::app_server::config::AppServerConfig;

pub(crate) enum ClientRequest<'a> {
    Initialize {
        config: &'a AppServerConfig,
    },
    ThreadStart {
        config: &'a AppServerConfig,
        cwd: String,
    },
    TurnStart {
        thread_id: &'a str,
        prompt: &'a str,
        effort: &'a str,
    },
    TurnInterrupt {
        thread_id: &'a str,
        turn_id: &'a str,
    },
}

impl ClientRequest<'_> {
    pub(crate) fn method(&self) -> &'static str {
        match self {
            Self::Initialize { .. } => "initialize",
            Self::ThreadStart { .. } => "thread/start",
            Self::TurnStart { .. } => "turn/start",
            Self::TurnInterrupt { .. } => "turn/interrupt",
        }
    }

    pub(crate) fn into_message(self, id: u64) -> Value {
        let method = self.method();
        match self {
            Self::Initialize { config } => json!({
                "id": id,
                "method": method,
                "params": {
                    "clientInfo": {
                        "name": config.client_name.as_str(),
                        "title": config.client_title.as_str(),
                        "version": config.client_version.as_str()
                    },
                    "capabilities": {
                        "experimentalApi": true
                    }
                }
            }),
            Self::ThreadStart { config, cwd } => {
                let mut message = json!({
                    "id": id,
                    "method": method,
                    "params": {
                    "approvalPolicy": config.approval_policy.as_str(),
                    "cwd": cwd,
                    "developerInstructions": config.developer_instructions.as_str(),
                    "ephemeral": true,
                    "sandbox": config.sandbox.as_str(),
                    "serviceName": config.service_name.as_str()
                    }
                });
                if let Some(model) = &config.model {
                    message["params"]["model"] = json!(model);
                }
                message
            }
            Self::TurnStart {
                thread_id,
                prompt,
                effort,
            } => json!({
                "id": id,
                "method": method,
                "params": {
                    "threadId": thread_id,
                    "effort": effort,
                    "input": [{ "type": "text", "text": prompt }]
                }
            }),
            Self::TurnInterrupt { thread_id, turn_id } => json!({
                "id": id,
                "method": method,
                "params": {
                    "threadId": thread_id,
                    "turnId": turn_id
                }
            }),
        }
    }
}

pub(crate) enum ClientNotification {
    Initialized,
}

impl ClientNotification {
    pub(crate) fn into_message(self) -> Value {
        match self {
            Self::Initialized => json!({
                "method": "initialized",
                "params": {}
            }),
        }
    }
}

pub(crate) struct ServerRequest {
    id: Value,
    kind: ServerRequestKind,
}

impl ServerRequest {
    pub(crate) fn from_message(message: &Value) -> Option<Self> {
        let id = message.get("id")?.clone();
        let method = message.get("method")?.as_str()?;
        Some(Self {
            id,
            kind: ServerRequestKind::from_method(method),
        })
    }

    pub(crate) fn method(&self) -> &str {
        self.kind.method()
    }

    pub(crate) fn decline_response(&self) -> Value {
        match &self.kind {
            ServerRequestKind::CommandExecutionApproval | ServerRequestKind::FileChangeApproval => {
                json!({ "id": self.id, "result": { "decision": "decline" } })
            }
            ServerRequestKind::ToolRequestUserInput => {
                json!({ "id": self.id, "result": { "answers": {} } })
            }
            ServerRequestKind::McpServerElicitation => {
                json!({ "id": self.id, "result": { "action": "decline" } })
            }
            ServerRequestKind::PermissionsApproval => {
                json!({
                    "id": self.id,
                    "result": {
                        "permissions": {
                            "fileSystem": null,
                            "network": null
                        },
                        "scope": "turn"
                    }
                })
            }
            ServerRequestKind::DynamicToolCall => {
                json!({
                    "id": self.id,
                    "result": {
                        "success": false,
                        "contentItems": [
                            {
                                "type": "inputText",
                                "text": "Dynamic tool calls are not supported by this client."
                            }
                        ]
                    }
                })
            }
            ServerRequestKind::ChatgptAuthTokensRefresh
            | ServerRequestKind::AttestationGenerate
            | ServerRequestKind::Unknown(_) => {
                json!({
                    "id": self.id,
                    "error": {
                        "code": -32601,
                        "message": format!("Unsupported server request: {}", self.kind.method())
                    }
                })
            }
        }
    }
}

enum ServerRequestKind {
    CommandExecutionApproval,
    FileChangeApproval,
    ToolRequestUserInput,
    McpServerElicitation,
    PermissionsApproval,
    DynamicToolCall,
    ChatgptAuthTokensRefresh,
    AttestationGenerate,
    Unknown(String),
}

impl ServerRequestKind {
    fn from_method(method: &str) -> Self {
        match method {
            "item/commandExecution/requestApproval" => Self::CommandExecutionApproval,
            "item/fileChange/requestApproval" => Self::FileChangeApproval,
            "item/tool/requestUserInput" => Self::ToolRequestUserInput,
            "mcpServer/elicitation/request" => Self::McpServerElicitation,
            "item/permissions/requestApproval" => Self::PermissionsApproval,
            "item/tool/call" => Self::DynamicToolCall,
            "account/chatgptAuthTokens/refresh" => Self::ChatgptAuthTokensRefresh,
            "attestation/generate" => Self::AttestationGenerate,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn method(&self) -> &str {
        match self {
            Self::CommandExecutionApproval => "item/commandExecution/requestApproval",
            Self::FileChangeApproval => "item/fileChange/requestApproval",
            Self::ToolRequestUserInput => "item/tool/requestUserInput",
            Self::McpServerElicitation => "mcpServer/elicitation/request",
            Self::PermissionsApproval => "item/permissions/requestApproval",
            Self::DynamicToolCall => "item/tool/call",
            Self::ChatgptAuthTokensRefresh => "account/chatgptAuthTokens/refresh",
            Self::AttestationGenerate => "attestation/generate",
            Self::Unknown(method) => method,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn known_server_requests_decline_with_protocol_shaped_results() {
        let cases = [
            (
                "item/fileChange/requestApproval",
                json!({ "id": 1, "result": { "decision": "decline" } }),
            ),
            (
                "item/tool/requestUserInput",
                json!({ "id": 1, "result": { "answers": {} } }),
            ),
            (
                "mcpServer/elicitation/request",
                json!({ "id": 1, "result": { "action": "decline" } }),
            ),
            (
                "item/permissions/requestApproval",
                json!({
                    "id": 1,
                    "result": {
                        "permissions": {
                            "fileSystem": null,
                            "network": null
                        },
                        "scope": "turn"
                    }
                }),
            ),
            (
                "item/tool/call",
                json!({
                    "id": 1,
                    "result": {
                        "success": false,
                        "contentItems": [
                            {
                                "type": "inputText",
                                "text": "Dynamic tool calls are not supported by this client."
                            }
                        ]
                    }
                }),
            ),
        ];

        for (method, expected) in cases {
            let request = ServerRequest::from_message(&json!({
                "id": 1,
                "method": method,
                "params": {}
            }))
            .unwrap();

            assert_eq!(request.decline_response(), expected);
        }
    }
}
