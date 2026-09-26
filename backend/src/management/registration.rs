use gateway_plugin_sdk::call::management::{
    ManagementPage, ManagementRegistration, ManagementResource, ManagementRoute,
};

pub fn registration() -> ManagementRegistration {
    ManagementRegistration {
        routes: [
            ("GET", "api/accounts"),
            ("POST", "api/account"),
            ("POST", "api/settings"),
        ]
        .into_iter()
        .map(|(method, path)| ManagementRoute {
            method: method.to_owned(),
            path: path.to_owned(),
            request_content_types: if method == "POST" {
                vec!["application/json".to_owned()]
            } else {
                Vec::new()
            },
            response_content_types: vec!["application/json".to_owned()],
        })
        .collect(),
        resources: ["web/index.html", "web/app.js", "web/app.css"]
            .into_iter()
            .map(|path| ManagementResource {
                path: path.to_owned(),
                public: false,
            })
            .collect(),
        pages: vec![ManagementPage {
            id: "state-observer".to_owned(),
            title: "State 观测".to_owned(),
            description: Some("账号与模型的业务 State 记录".to_owned()),
            entry: "web/index.html".to_owned(),
            icon: None,
        }],
        callbacks: Vec::new(),
    }
}
