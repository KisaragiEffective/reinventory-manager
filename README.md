# neosvr-inventory-management
## Notes to foreign users
* This readme is written in Japanese. Please use external tool to translate this document into your language. I'm sorry for inconvenience.
* If this document is translated into a language other than Japanese, the Japanese version will prevail to the extent that there is any conflict.

## これはなに？
[NeosVR](https://neos.com)用のインベントリ整理支援ツールです。現在はベータ版となっています。

## なぜ？
私が知る限りでは、NeosはWindowsの「エクスプローラー」にあるようなディレクトリ間の移動ができません。
話を聞いていく中で、「大晦日にインベントリを整理するイベントがある」「アイテムを全部だして選り分ける必要がある」という事例を聞きました。
また、開発者である私も、ゲーム内でインベントリにアイテムをセーブする中で、インベントリ直下のアイテムがすでに8列ほどとなっていました。このペースで保存していくと必然的に直下が大変なカオスを招くことになるだろうと予想したため、今回ツールの作成に踏み切りました。

## ダウンロード
ダウンロードの前に[下記](#ライセンス)をお読みください。

以下のOS向けに予めコンパイルされたバイナリが配布されています。
* Windows: *TODO*
* Linux: *TODO*
* macOS: *TODO*

もし対応していないなど、何らかの事情でセルフコンパイルが必要な場合、`git clone`からこのレポジトリをクローンしてソースコードを入手してください。

## 使用上の注意
* すでにクライアントでログインしているときにこのツールを使うと副作用が起こります。これらはすべてマルチログインできないという技術的な問題であるそうです (by @kazu0617)。
  * インベントリからアイテムを取り出せなくなる
  * インベントリのシンク(Sync)が一向に終わらない
  * 再起動したときにRemember Meを有効にしていてもログアウトされた状態になる
* `cargo run`経由で動かす場合、`cargo`のフラグと本プロダクトのフラグを`--`で区切ってください。
  * 例: `cargo run --release -- -e kisaragi.marine@gmail.com -p ************** list Inventory`

## コントリビューション
バグ報告、デバッグ、パッチの送信、ドキュメントの誤字修正など、いかなる形でもコントリビューションをいただければ幸いです。
このツールはRustで書かれています。
パッチを送っていただいた際、別途の表明がない限りは、下記[ライセンス](#ライセンス)においてパッチを取り扱わせていただきます。

## ライセンス
* `src`ディレクトリ以下のソースコード、ソースコードに付随するドキュメント、及び配布されるバイナリはRust本体のライセンスに合わせて、MITライセンスとApache License, Version 2.0 (SPDX: `MIT OR Apache-2.0`) とします。
  * このライセンスはデュアルライセンスであり、あなたはどちらかのライセンスを選ぶことができます。
* README.md、Cargo.lock、Cargo.tomlはCC0とします。
* ファイルの中に合理的な方法でライセンスが表示されている場合は、そのファイルはそのライセンスによってライセンスされています。
* その他のファイルについては著作権を留保します。

### 免責事項
* MIT License及びApache License, Version 2.0 §7に指定されている通り、成果物は現状のまま提供されるものとし、開発者およびコントリビューターは当人が提供する部分についていかなる保証も提供しません。
  * 特に、このツールを使用して目的が達成されることをいかなる形態でも保障しません。
  * 特に、このツールを使用してユーザーにアイテム消失などの損害が起きないことを保障しません。
  * 特に、内部APIの破壊的変更に追従することを保障しません。

## 開発者
* KisaragiEffective (IGN: `kisaragi marine`)

## スペシャルサンクス
(敬称略)
* kazu0617
