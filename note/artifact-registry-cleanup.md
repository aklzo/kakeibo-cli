# Artifact Registry のクリーンアップポリシー設定

## 概要

GitHub Actions はデプロイのたびに Artifact Registry へイメージを push するため、放っておくとイメージが増え続ける。
Artifact Registry の**クリーンアップポリシー**を使い、`kakeibo` リポジトリには**最新 5 個のイメージだけを残し、それより古いものを自動で削除する**ようにした。

- 設定日: 2026-11-03
- 対象: プロジェクト `kakeibo-495400` / リージョン `asia-northeast1` / リポジトリ `kakeibo`（パッケージは `kakeibo-api` のみ）
- 方法: gcloud CLI

---

## 現在の設定

```json
[
  {
    "name": "delete-all",
    "action": {"type": "Delete"},
    "condition": {"tagState": "any"}
  },
  {
    "name": "keep-latest-5",
    "action": {"type": "Keep"},
    "mostRecentVersions": {"keepCount": 5}
  }
]
```

| ポリシー | 意味 |
|---------|------|
| `delete-all` | タグの有無にかかわらず、すべてのイメージを削除対象にする |
| `keep-latest-5` | パッケージごとに最新 5 個を残す |

削除ポリシーと保持ポリシーの両方に当てはまるイメージは**保持が優先**される。
そのため「全部を削除対象にして、最新 5 個だけ保持で守る」という組み合わせで、「最新 5 個を残す」が実現できる。

---

## 設定手順

### ① ポリシーファイルを作る

上の JSON を `cleanup-policy.json` として保存する（リポジトリに置く必要はない）。

### ② 適用する

```bash
gcloud artifacts repositories set-cleanup-policies kakeibo \
  --project=kakeibo-495400 \
  --location=asia-northeast1 \
  --policy=cleanup-policy.json \
  --no-dry-run
```

`--no-dry-run` を付けると実際に削除される。`--dry-run` にすると、削除はせずに対象を記録するだけになる（後述の注意点を参照）。

### ③ 設定を確認する

```bash
gcloud artifacts repositories list-cleanup-policies kakeibo \
  --project=kakeibo-495400 \
  --location=asia-northeast1
```

出力の 1 行目に `Dry run is disabled.` と出ていれば、実際に削除される状態になっている。

### ④ 削除されたことを確認する（適用から約 1 日後）

```bash
gcloud artifacts docker images list \
  asia-northeast1-docker.pkg.dev/kakeibo-495400/kakeibo \
  --include-tags --sort-by=~createTime
```

---

## 実例（2026-11-03 の適用時点）

適用前のイメージは 7 個（合計約 149MB）だった。

| 作成日時（JST） | タグ（コミット） | 使っている Cloud Run リビジョン | 適用後 |
|---------------|----------------|-------------------------------|-------|
| 2026-10-29 02:25 | `9891700` / `latest` | 00011（稼働中） | 残る |
| 2026-05-21 02:24 | `9a83b27` | 00010 | 残る |
| 2026-05-05 20:46 | `c1b670f` | 00009 | 残る |
| 2026-05-05 20:36 | `1bfdd8d` | 00008 | 残る |
| 2026-05-05 14:13 | `fc937ae` | 00007 | 残る |
| 2026-05-05 13:45 | `ec1f88a` | 00006 | **削除** |
| 2026-05-05 10:07 | （タグなし） | 00001〜00005 | **削除** |

タグなしのイメージは、初回に `gcloud builds submit` で `latest` として push したもの。
その後のデプロイで `latest` タグが新しいイメージへ移ったため、タグなしになった。

削除されると、リビジョン 00001〜00006 にはロールバックできなくなる（後述）。

---

## 詰まった点・分かりにくかった点

実際にエラーになった箇所はなかった。ただし、公式ドキュメントだけでは分かりにくい点がいくつかあった。

### 1. 保持ポリシーだけでは何も削除されない

「最新 5 個を残す」だけを書けばよさそうに見えるが、保持ポリシーは「削除対象から守る」ためのもので、それ自体は何も削除しない。
削除ポリシー（`delete-all`）と組み合わせて初めて古いものが消える。

### 2. 適用してもすぐには削除されない

クリーンアップはバックグラウンドで定期的に実行され、反映まで**最大で 1 日程度**かかる。
適用直後にイメージ一覧を見ても、まだ 7 個のままなのは正常。

### 3. dry-run の結果は監査ログにしか出ない

`--dry-run` で適用すると削除はされないが、「何が削除されるはずだったか」は Cloud Audit Logs のデータアクセスログ（`DATA_WRITE`）にしか出ない。
このログは既定で無効なので、dry-run の結果を見るにはログの設定から変える必要がある。

今回は dry-run を使わず、`gcloud artifacts docker images list` で一覧を作成日時順に並べ、「上から 5 個が残り、残りが消える」ことを事前に確認してから `--no-dry-run` で適用した。
イメージ数が少ないリポジトリならこの方法で十分。

### 4. 「最新」の基準がドキュメントに明記されていない

`mostRecentVersions` が作成日時と更新日時のどちらで「最新」を判定するのかは、ドキュメントに書かれていない。
`latest` タグが移ると、移った元のイメージの更新日時も変わる点に注意する。
今回の 7 個は、どちらの順で並べても削除対象の 2 個が同じだったので問題にならなかった。

### 5. Cloud Run の古いリビジョンへ戻せなくなる

Cloud Run の各リビジョンは、イメージを digest（`sha256:...`）で参照している。
イメージを削除すると、そのイメージを使うリビジョンにはトラフィックを戻せなくなる。

- 稼働中のリビジョンのイメージは、常に最新 5 個に含まれるので消えない（デプロイ = 最新の push のため）
- 「何世代前まで戻せるようにしたいか」で `keepCount` を決める。5 個なら 4 世代前まで戻せる

### 6. gcloud の出力は大文字になる

設定ファイルでは `"Delete"` や `"any"` と書くが、`list-cleanup-policies` の出力では `"DELETE"`、`"ANY"` と大文字で表示される。
表示が違うだけで、設定が書き換わったわけではない。
設定ファイルは公式ドキュメントの書き方（先頭だけ大文字、`any` は小文字）に合わせておく。

---

## 変更・解除する場合

`keepCount` を変えるときは、JSON を書き換えて `set-cleanup-policies` をもう一度実行する。
同じ名前のポリシーは新しい内容で上書きされる想定だが、ファイルに書かなかった既存ポリシーが残るかどうかは未検証なので、変更後は必ず `list-cleanup-policies` で確認すること。

ポリシーを外す場合:

```bash
gcloud artifacts repositories delete-cleanup-policies kakeibo \
  --project=kakeibo-495400 \
  --location=asia-northeast1 \
  --policynames=delete-all,keep-latest-5
```

> **注意**: `keep-latest-5` だけを外して `delete-all` を残すと、すべてのイメージが削除対象になる。外すときは両方まとめて外すこと。

---

## 参考

- [クリーンアップ ポリシーを構成する（Artifact Registry）](https://cloud.google.com/artifact-registry/docs/repositories/cleanup-policy)
- [Google Cloud 無料枠](https://cloud.google.com/free/docs/free-cloud-features)（Artifact Registry は 0.5GB/月まで無料）
