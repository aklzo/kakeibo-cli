use std::fmt;
use std::str::FromStr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// 取引カテゴリ。CLIの識別子・DB保存値として使用する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Fixed,
    Subscription,
    Food,
    Daily,
    Transport,
    Clothing,
    Medical,
    Beauty,
    Social,
    Special,
    Learning,
    Hobby,
    Interior,
    Income,
}

impl Category {
    /// 画面表示用の日本語名を返す。
    pub fn display_name(&self) -> &'static str {
        match self {
            Category::Fixed => "固定費",
            Category::Subscription => "サブスク",
            Category::Food => "食費",
            Category::Daily => "日用品",
            Category::Transport => "交通費",
            Category::Clothing => "被服費",
            Category::Medical => "医療費",
            Category::Beauty => "美容費",
            Category::Social => "交際費",
            Category::Special => "特別日",
            Category::Learning => "学習",
            Category::Hobby => "趣味",
            Category::Interior => "インテリア費",
            Category::Income => "収入",
        }
    }

    /// 収入カテゴリかどうかを返す。
    pub fn is_income(&self) -> bool {
        matches!(self, Category::Income)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Category::Fixed => "fixed",
            Category::Subscription => "subscription",
            Category::Food => "food",
            Category::Daily => "daily",
            Category::Transport => "transport",
            Category::Clothing => "clothing",
            Category::Medical => "medical",
            Category::Beauty => "beauty",
            Category::Social => "social",
            Category::Special => "special",
            Category::Learning => "learning",
            Category::Hobby => "hobby",
            Category::Interior => "interior",
            Category::Income => "income",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Category {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fixed" => Ok(Category::Fixed),
            "subscription" => Ok(Category::Subscription),
            "food" => Ok(Category::Food),
            "daily" => Ok(Category::Daily),
            "transport" => Ok(Category::Transport),
            "clothing" => Ok(Category::Clothing),
            "medical" => Ok(Category::Medical),
            "beauty" => Ok(Category::Beauty),
            "social" => Ok(Category::Social),
            "special" => Ok(Category::Special),
            "learning" => Ok(Category::Learning),
            "hobby" => Ok(Category::Hobby),
            "interior" => Ok(Category::Interior),
            "income" => Ok(Category::Income),
            _ => Err(anyhow!("不明なカテゴリです: {s}")),
        }
    }
}

/// DBから取得した取引レコード。
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    /// 自動採番ID（削除後は欠番）
    pub id: i64,
    pub name: String,
    /// 金額（円、正の整数）
    pub amount: i64,
    /// 日付（YYYY-MM-DD）
    pub date: String,
    pub category: Category,
    pub memo: Option<String>,
    /// 登録日時（ISO 8601）
    pub created_at: String,
}

/// DBから取得した予算レコード。
#[derive(Debug, Serialize, Deserialize)]
pub struct Budget {
    pub id: i64,
    /// 対象月（YYYY-MM）。NULL の場合は全月共通のデフォルト予算。
    pub month: Option<String>,
    /// 対象カテゴリ。NULL の場合は月全体の予算。
    pub category: Option<Category>,
    /// 上限金額（円）
    pub amount: i64,
}
