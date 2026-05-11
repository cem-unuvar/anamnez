This file maps the KVKK obligations that apply to anamnez onto the architecture in `README.md`.

It is written for the MVP described there: a single Mac Studio per clinic on the clinic's LAN, no public exposure, local OCR / transcription / LLM in production, OpenRouter only in `ENV=TEST`. Where an obligation falls on the *clinic* (the data controller) rather than the *software*, that is called out explicitly — anamnez cannot make a clinic KVKK-compliant on its own, but it must not block them from being compliant either, and it must do the parts that only the software can do.

Legal references used throughout:

- **KVKK** — 6698 sayılı Kişisel Verilerin Korunması Kanunu, as amended by 7499 sayılı Kanun (Resmî Gazete 12.03.2024, yür. 01.06.2024).
- **KSV Yönetmeliği** — Kişisel Sağlık Verileri Hakkında Yönetmelik (RG 21.06.2019, son değ. RG 03.12.2025).
- **Silme/İmha Yönetmeliği** — Kişisel Verilerin Silinmesi, Yok Edilmesi veya Anonim Hale Getirilmesi Hakkında Yönetmelik (RG 28.10.2017).
- **Yurt Dışı Yönetmeliği** — Kişisel Verilerin Yurt Dışına Aktarılmasına İlişkin Usul ve Esaslar Hakkında Yönetmelik (RG 10.07.2024, yür. 10.07.2024).
- **Aydınlatma Tebliği** — Aydınlatma Yükümlülüğünün Yerine Getirilmesinde Uyulacak Usul ve Esaslar Hakkında Tebliğ (RG 10.03.2018).
- **2018/10** — KVK Kurulu, 31.01.2018 t. ve 2018/10 sayılı Karar — özel nitelikli kişisel verilerde yeterli önlemler.
- **2019/10** — KVK Kurulu, 24.01.2019 t. ve 2019/10 sayılı Karar — veri ihlali bildiriminde 72 saat.
- **Hasta Hakları Yönetmeliği** — RG 01.08.1998, hâlen yürürlükte (son değ. 2014).
- **TCK** — 135 (kaydetme), 136 (hukuka aykırı verme/ele geçirme), 137 (kamu görevlisi / meslek artırımı).

## 1. Roller — kim veri sorumlusu, kim veri işleyen

| Aktör | Rol | Neden |
|---|---|---|
| Klinik (deployment'ı satın alan tüzel kişi / muayenehane sahibi hekim) | **Veri sorumlusu** | İşleme amaçlarını ve vasıtalarını belirleyen taraf. Hastayla doğrudan ilişkili olan taraf. |
| Klinik içindeki hekim/hemşire kullanıcılar | Veri sorumlusu adına işleyen çalışan | Veri sorumlusu organizasyonu içinde, sır saklama yükümlülüğü altında. |
| Anamnez (biz, yazılım sağlayıcı) — MVP'de | **Yazılım tedarikçisi** (veri işleyen DEĞİL) | MVP'de Mac Studio klinikte; biz çalışma anında veriye erişmiyoruz, telemetri yok, uzak destek yok. |
| Anamnez (biz) — uzak destek/güncelleme alınırsa | **Veri işleyen** olur | O an itibariyle yazılı sözleşme (KVKK m. 12/2 atfıyla m. 12 ve Kurulu rehberi) ve sır saklama şart. |
| Apple (OS), donanım tedarikçisi | İlgisiz (veriye mantıksal erişim yok) | Disk şifreleme açıkken Apple'ın veriye erişimi yoktur; bulut yedek kapalı. |
| OpenRouter ve barındırılan LLM sağlayıcıları | **YALNIZCA `ENV=TEST` rejiminde** ilgili; üretimde HİÇ aktif değil | §10. |

Bu tablo `README.md`'nin "Deployment" bölümüyle tutarlı olmalı: Mac Studio kliniğe ait, dışarıdan erişilemez, sertifika klinik içinde üretilir. Biz yazılımı imzalayıp dağıtırız; ürettiğimiz tek şey statik binary'lerdir.

**Çıkarım — sözleşmeleme:** Klinikle yapılan tedarik sözleşmesinde (a) bizim MVP'de veri işleyen olmadığımız, (b) destek/uzak müdahale gerektiren bir vaka çıkarsa o işlem için ayrı bir KVKK m. 12 kapsamlı veri işleme sözleşmesi (DPA) imzalanacağı, (c) ne biçim telemetri toplandığı (varsa) açıkça yazılı olmalı. Klinik buna kendi VERBİS / işleme envanteri için ihtiyaç duyar.

## 2. Hukuki dayanak — sağlık verisi neye dayanılarak işleniyor?

Sağlık verisi KVKK m. 6/1 anlamında **özel nitelikli kişisel veri**dir. m. 6/2 işlemeyi yasaklar; m. 6/3 istisnalar getirir.

Anamnez'deki tüm klinik işleme için dayanak **m. 6/3'ün sağlık istisnası**dır:

> "...kamu sağlığının korunması, koruyucu hekimlik, tıbbi teşhis, tedavi ve bakım hizmetlerinin yürütülmesi, sağlık hizmetleri ile finansmanının planlanması ve yönetimi amacıyla sır saklama yükümlülüğü altında bulunan kişiler veya yetkili kurum ve kuruluşlar tarafından açık rıza aranmaksızın işlenebilir."

Bu, sağlık hizmeti sunan klinikte hekim ve sağlık personeli için **açık rıza gerektirmez**. Üç şart birlikte sağlanmalı:
1. Amaç gerçekten teşhis/tedavi/bakım olmalı — pazarlama, profilleme, dış araştırma değil.
2. İşleyen taraf sır saklama yükümlüsü olmalı (hekim ✓, tıbbi sekreter çalışan sıfatıyla ✓).
3. Veri amaçla sınırlı, ölçülü olmalı (KVKK m. 4).

**KVK Kurulu emsalleri** açıktır:
- Sağlık hizmetinin sunulması için **açık rıza talep edilmesi başlı başına aykırılıktır** (2023/692 — özel sağlık kuruluşu hizmeti açık rıza şartına bağlamış, ceza). m. 6/3 zaten yeterli dayanak; bunun üstüne rıza istemek "rızanın özgürce verildiği" karinesini zedeler.
- Hastane reklam/tanıtım için sağlık verisi işleyemez, **açık rıza alsa bile** (2023/787) — Özel Hastaneler Yönetmeliği m. 60 reklamı yasaklar; KVKK m. 4'ün amaca uygunluk ilkesini ihlal eder.

**Açık rıza'nın gerektiği vakalar (anamnez için):**
- Klinik dışı amaçlarla aktarım (örn. bir hastanın verisini başka bir hekime/uzmana referans için göndermek — bu da m. 6/3 kapsamına girebilir ama her vaka değerlendirilmeli).
- Bilimsel / istatistiksel kullanım için **veri anonim değilse** (anonimleştirildiyse rıza şart değil, KSV Yönetmeliği m. 16).
- Hastanın dosyasını avukatına aktarma (KSV Yönetmeliği m. 10 atfıyla — genel vekâletname yetmez, özel açık rıza beyanı şart).

## 3. Anamnez'in m. 6/3 dayanağını teknik olarak korumak

`README.md` mimarisi bu dayanağı çoktan koruyor; aşağıdakileri **bozmamak** kritik:

1. **Sır saklama yükümlüsü olmayanlara erişim verilmemeli.** `user` tablosundaki her hesap, klinik içinde sır saklama yükümlüsü bir kişiye karşılık gelmek zorunda. Admin onboarding akışı bunu kullanıcı sözleşmesi (gizlilik taahhütnamesi) imzasıyla sürece dahil etmeli — bu klinik sorumluluğu olsa da admin UI'ında bir "kullanıcı eklerken gizlilik sözleşmesi onayı verildi mi?" zorunlu kutusu olmalı.
2. **Veri minimizasyonu.** Observation şeması (`README.md` §Data Modelling) zaten gerekli alanları içeriyor; "ekstra" alan eklenmemeli. Manuel girişte serbest metin uyarısı: hastayla doğrudan ilgili olmayan veriyi (örn. başka bir hastanın adı) girmemek için.
3. **Amaç sapmasını engellemek.** İkinci kullanım (pazarlama, tanıtım, dış araştırma) anamnez'in özelliği olarak **kesinlikle yer almamalı**. Eklenirse m. 4 ve 2023/787 emsal kararı kapsamında ihlal doğar.

## 4. `patient_access` ACL'nin KVKK karşılığı

KVKK m. 4 ölçülülük + 2018/10 erişim kapsamının net tanımlı olması şartı + KSV Yönetmeliği m. 5/6 "verilecek olan sağlık hizmetinin gereği ile sınırlı" şartı — bunların yazılım karşılığı `patient_access` tablosu.

Mevcut tasarım (owner / collaborator / read_only) ölçü kuralını karşılıyor. Eklenmesi gereken:

- **Periyodik yetki gözden geçirme** (2018/10): admin UI'da "X aydır erişimi olan ama Y aydır kullanmamış" raporu. En azından 6 aylık otomatik bir gözden geçirme görevi.
- **Anında iptal** (2018/10): kullanıcı `disabled_at` set edildiğinde tüm `patient_access` satırları derhal etkisiz. Mevcut session token'ları derhal geçersiz. Bu yazılı bir gerekliliktir — sadece `disabled_at` yetmez, aktif session'lar revoke edilmeli.
- **Erişim sebebi opsiyonel kayıt**: bir kullanıcı normalde erişimi olmayan bir hastaya erişim aldığında (örn. acil), `audit_log.metadata` içinde sebep prompt'u. Acil senaryolarda "kırılgan kapı" (break-glass) deseni, m. 6/3 acil teşhis durumlarını destekler ama denetim izi şart.

## 5. Audit log — KVKK perspektifi

`README.md`'deki append-only audit_log iyi başlangıç. KVKK açısından zorunlu olan ve karşılanması gereken kayıt türleri:

- Her başarılı ve başarısız oturum açma denemesi (m. 12 + 2018/10 §çalışan güvenliği).
- Her hasta kaydı görüntülemesi (`patient.view`).
- Her observation create/amend (`observation.create`, `observation.amend`) — diff dahil.
- Her `patient_access` değişikliği (grant/revoke/level change).
- Her veri ihracı (export). MVP'de PDF export, ZIP export, ekrandan veri kopyalama UI tarafında loglanabilir değildir, ama sunucu tarafı her büyük okumayı loglar.
- Her silme/yok etme operasyonu (Silme/İmha Yönetmeliği m. 7 — imha kayıtları en az 3 yıl saklanır).
- Her admin işlemi (kullanıcı oluşturma, devre dışı bırakma, role değişikliği).
- Her enrollment string üretimi ve kullanımı (workstation ekleme — fiziksel anahtar dağıtımı izi).

**Audit log'un kendisi de kişisel veridir** (m. 5 atfıyla). Saklama süresi sınırsız değildir; bizim kararımız: audit logları **10 yıl** sakla. Gerekçe: hekimlik standardı tıbbi kayıt saklama süresi 20 yıl (KSV Yönetmeliği m. 11 ölü kişi verileri için bu eşiği koyuyor), audit log'un yarı ömrünü buna oranlamak makul; ilgili kişinin haklarını kullanması ve adli süreçler için yeterli; sonsuza dek saklamak gerekli değil. Bu süreyi `config` üzerinden klinik bazında değiştirilebilir kılmak.

**Audit log mutasyon edilemezliği:** append-only "uygulama katmanında" olduğu `README.md`'de yazılı; bunu DB seviyesinde de zorlamak gerekli — SQLite kullanıyorsak `audit_log` üzerinde sadece INSERT yapan bir prepared statement; UPDATE/DELETE içeren herhangi bir kod yolu `cargo deny` veya custom lint'le yasaklanmalı. Sızıntı senaryosunda audit logu inkâr edilemez kılmak için periyodik bir merkle/hash zinciri (her gece son bloğun hash'i ile imzalama) faydalı; MVP için zorunlu değil ama belgelendirilsin.

## 6. Madde 12 — teknik ve idari tedbirler (TOM)

Aşağıdaki tablo 2018/10 ve Kişisel Veri Güvenliği Rehberi'ndeki başlıkları anamnez'in mevcut tasarımına eşliyor.

| Yükümlülük | Anamnez nerede karşılıyor | Eksik / klinik sorumluluğu |
|---|---|---|
| Kriptografik depolama | Mac Studio FileVault zorunlu; SQLite dosyası FileVault altında. Ek olarak DB seviyesinde [SQLCipher](https://www.zetetic.net/sqlcipher/) veya benzeri page-level encryption — anahtar Keychain + Secure Enclave'de. | Yedekleme şifresi (§7). |
| Anahtar farklı ortamda | DB anahtarı Secure Enclave; recovery code first-boot'ta fiziksel yazdırılıyor (zaten README'de). | Recovery code'un saklanma şekli — klinik admin'in fiziksel kasası. Doc. |
| İletim güvenliği | Server self-signed TLS + workstation fingerprint pinning (zaten README'de). | — |
| Erişim yetki matrisi | `user`.`role` + `patient_access`. | Periyodik review (§4). |
| İki kademeli kimlik doğrulama (uzak erişim için) | MVP uzak erişimi reddediyor — gereksinim düşüyor. Headscale eklenirse şart olur. | Belgelendirme. |
| Log'lama | `audit_log` (§5). | Anomali tespiti — basit kural seti, MVP'de manuel. |
| Sızma testi / güvenlik testleri | Layer 1-3 testleri var; ek olarak yıllık üçüncü taraf pentesti tavsiye. | Klinik veya bizim tarafımızda planla. |
| Çalışan eğitimi | — | Tamamen klinik sorumluluğu. Onboarding paketinde KVKK + sır saklama eğitim materyali sağlanması iyi olur ama yasal yükümlülük değil. |
| Çalışan gizlilik sözleşmeleri | — | Klinik sorumluluğu. |
| Fiziksel güvenlik | Mac Studio'nun fiziksel yeri | Klinik sorumluluğu — kilitli serverroom/dolap tavsiyesi belgelensin. |
| Yedekleme | MVP'de TBD — `README.md`'de açık değil | **Bu boşluk doldurulmalı:** otomatik gece yedeği, AES-256 ile şifreli, anahtar Secure Enclave'de; harici disk veya başka bir klinik içi cihaz hedefi. Bulut yedek varsayılan kapalı. |
| Veri sınıflandırma | Observation şeması zaten kategori taşıyor (LOINC/SNOMED kodu) | İlerideki "hassas" alt-etiketleme (psikiyatri, HIV vb. ekstra sıkı erişim) MVP sonrası. |
| OS güncellemeleri | macOS otomatik update | İlk boot wizard'da otomatik update zorunlu, doğrula. |
| Antivirüs / EDR | macOS XProtect yeterli minimal düzey | Klinik talep ederse uyumlu EDR ekleme yolu açık olsun. |

## 7. Aydınlatma yükümlülüğü (m. 10 + Aydınlatma Tebliği)

Klinik her hastaya, veri elde edilirken **aydınlatma metni** sunmak zorunda. Tebliğ'in (RG 10.03.2018) zorunlu içeriği:

1. Veri sorumlusunun (kliniğin) kimliği.
2. Hangi amaçlarla işleneceği — anamnez bağlamında: teşhis, tedavi, bakım, klinik kayıt, faturalandırma, yasal yükümlülükler.
3. Kimlere ve hangi amaçla aktarılabileceği — örn. SGK, MHRS/e-Nabız (Sağlık Bakanlığı), sevk edilen başka bir hekim, hastanın talebiyle hukuki süreçler.
4. Toplama yöntemi ve hukuki sebebi — sözlü/yazılı anamnez, OCR yoluyla geçmiş raporlar, sesli kayıttan transkripsiyon (bunun belirtilmesi şart — hasta sesinin kaydedilebileceğini bilmeli), AI ile çıkarım yapıldığı (KVKK Üretken YZ Rehberi 11/2025 bunu netleştirdi).
5. KVKK m. 11 kapsamındaki haklar.

**Anamnez sorumluluğu:** Klinik için boş bırakılan alanlarla şablon bir aydınlatma metni sağla (admin UI'da kliniğin kendi adına/adresine göre doldurabileceği). Uygulamanın "kayıt yapıyorum" diye yüksek sesli uyarı çıkarması (transkripsiyon başlatılırken UI'da görünür şerit) iyi pratik.

**Sözlü aydınlatma da yeterlidir** (Tebliğ m. 5), ama yazılı belge ile destekli olması ispat açısından kritik. Klinik bunu hasta dosyasında saklamalı; anamnez'in bunu kabul tarihiyle birlikte `patient.consent_acknowledgments` gibi bir tabloda kayıt altına alabilmesi gerekli.

## 8. Açık rıza — sadece m. 6/3 dışında

Anamnez **açık rıza'yı klinik işleyişin önkoşulu yapmamalı.** 2023/692 emsal kararı net: hizmeti açık rıza şartına bağlamak başlı başına ihlaldir.

Açık rıza'nın gerçekten gerekeceği yerler:
- Verinin (anonim olmadan) bilimsel araştırma için kullanılması.
- Verinin klinik dışı pazarlama/iletişim için kullanılması (anamnez bunu desteklemeyecek — §3).
- Hastanın avukatına dosya teslimi (KSV Yönetmeliği m. 10).
- Hastanın yakın bilgilendirilmesinin Hasta Hakları Yönetmeliği m. 18 kapsamı dışına çıkması.

Yazılım gereksinimi: rıza var/yok ve hangi amaç için verildiği `patient_access` veya ayrı bir `consent` tablosunda izlenmeli. Açık rıza ne zaman geri çekildiyse zaman damgalı kayıt tutulmalı (KVKK m. 5/1 + Kurulu yorumu — rıza her zaman geri alınabilir).

## 9. VERBİS kayıt yükümlülüğü

KVK Kurulu kararıyla: ana faaliyeti özel nitelikli veri işlemek olan veri sorumluları **yıllık çalışan sayısı 10'dan az VE yıllık mali bilanço 10 milyon TL altında** ise VERBİS kaydından muaftır. Eşiklerin "her ikisi" şarttır.

Pratik sonuç anamnez müşterileri için:
- **Tek hekim muayenehanesi, küçük poliklinik** → muafiyet kapsamında olabilir. Yine de kayıt yapmak yasal risk azaltır.
- **Çok hekimli klinik / orta-büyük poliklinik** → VERBİS kaydı **zorunlu**.

Bu klinik kararı, anamnez kararı değil. Ama: anamnez **işleme envanteri (Veri İşleme Envanteri / Kişisel Veri İşleme Envanteri)** üretimini kolaylaştırmak için bir admin raporu sağlamalı — VERBİS bildirimi için klinik bunu kullanır. Rapor şunları içerir: veri kategorileri (kimlik, iletişim, sağlık, finans), veri sahibi grupları (hasta, çalışan), işleme amaçları, alıcı grupları, saklama süresi, varsa yurtdışı aktarım. Bu büyük ölçüde sabit şablondan üretilebilir (anamnez işleyişi standart).

## 10. Yurt dışına aktarım — `ENV=TEST` ve OpenRouter

`README.md` §Privacy net: "OCR ve transkripsiyon HER ZAMAN local. Sadece ENV=TEST'te LLM için OpenRouter."

KVKK m. 9 (7499 ile değişik) hiyerarşisi:
1. **Yeterlilik kararı**: Kurul'un yeterlilik kararı verdiği bir ülkeye/sektöre aktarım — Kurul henüz hiçbir ülkeyi onaylamadı. Mevcut.
2. **Uygun güvenceler**: standart sözleşme (5 iş günü içinde Kurum'a bildirim şart), bağlayıcı şirket kuralları, Kurum onaylı taahhütname.
3. **Arızi (occasional) istisnalar**: açık rıza dahil — ama Kurul yönetmeliği bunu artık "süreklilik arz etmeyen" haller için istisnalaştırdı; rutin operasyonel aktarımlara açık rıza dayanağı **kullanılamaz**.

OpenRouter ABD'de barındırılır ve sağlık verisi (özel nitelikli) için yeterlilik kararı yok. Standart sözleşme imzalama yolu teorik olarak mümkün ama gereksiz risk.

**Anamnez gereksinimi — sert kuralla zorlanmalı:**

- `ENV=TEST` `serde` ile doğrulanmış bir enum (`Environment::Test | Environment::Production`). Default = `Production`. Yanlış yazımda crash.
- `Environment::Test` aktif olduğunda:
  - UI'da kırmızı / belirgin bir "TEST" şeridi (zaten README'de yazıyor — bu KVKK gerekçesi ile vurgulanmalı).
  - Tüm hasta isim alanları otomatik olarak fake jenerasyonla doldurulmuş veya `[TEST]` ön ekli kabul edilir.
  - **Production database backup'ı `ENV=TEST` çalışan bir binary'ye yüklenemez.** DB dosyası baş tarafında bir "production" marker'ı; test binary onu açmaya çalışırsa panic.
  - OpenRouter çağrıları yalnızca bu modda mümkün; her çağrıdan önce prompt'un kaynak verisinin `test_dataset` flag'iyle işaretli olduğunun in-process kontrolü.
- `Environment::Production`'da OpenRouter slug'ı içeren bir konfigürasyon = config validation startup hatası, panic. README'nin "configuration as a first-class citizen" prensibi tam burada.

**Sonuç:** Anamnez **üretimde sıfır yurt dışı aktarım** yapan bir mimaridir. Bu, m. 9'un tüm karmaşıklığını ortadan kaldırır. Klinik tarafında VERBİS / işleme envanteri "yurt dışı aktarım yapılmamaktadır" şeklinde belgelenir.

## 11. Üretken yapay zekâ — LLM kullanımı

KVKK 2024 Kasım'da "Sohbet Robotları (ChatGPT) Bilgi Notu", 2025 Kasım'da Üretken YZ Rehberi (Yayın 113) yayımladı. Klinik kullanım için sonuçlar:

- Üçüncü taraf hosted LLM'e (ChatGPT, Gemini, Claude.ai gibi) **hasta verisi prompt'u girmek başlı başına aktarım sayılır.** Üretimde kapalı (§10).
- Yerel LLM kullanımı tercih edilen yöntem — anamnez zaten bu yönde (MLX inference Mac Studio'da).
- AI ile çıkarım yapıldığı hastanın aydınlatma metninde belirtilmeli (§7 madde 4).
- AI çıktısı **denetimsiz klinik karar olarak kullanılamaz** — hekim her observation'ı doğrulamalı. `observation` şemasındaki `status` (`preliminary | final | amended`) ve `extracted_by` + `confidence` alanları bunun için tasarlanmış; UI'da `extracted_by = 'llm'` olan kayıtlar hekim "final"e çevirmedikçe görsel olarak ayırt edilebilir kalmalı.
- LLM "committee" özelliği (`README.md` §Analysis): her komite üyesinin çıktısı `audit_log`'a eklenmeli ve "bu analiz LLM tarafından üretilmiştir, klinik karar yerine geçmez" şerhi UI'da kalıcı görünür.

KVKK m. 11/g: ilgili kişinin "münhasıran otomatik sistemlerle analiz edilmesi suretiyle kişinin kendisi aleyhine bir sonucun ortaya çıkmasına itiraz etme" hakkı vardır. Anamnez'in tasarımında karar hekime aittir, otomatik nihai karar yoktur — bu hak çiğnenmez. Ama açıkça belgelensin.

## 12. İlgili kişi (hasta) hakları — m. 11 + Hasta Hakları Yönetmeliği

Klinik 30 gün içinde yanıt vermek zorunda. Yazılım, kliniğin bu süreyi tutturmasını mümkün kılmalı.

Anamnez'in desteklemesi gereken hasta hakları akışları:

| Hak | Madde | Yazılım gereksinimi |
|---|---|---|
| Verisinin işlenip işlenmediğini öğrenme | KVKK m. 11/a | Bir hastanın klinikteki tüm kayıtları için "hasta görünüm" — admin/owner kullanıcı için. |
| İşlenmişse bilgi talep etme | KVKK m. 11/b | Hasta dosyası PDF export (bütün observations + source documents + extractions). Hasta Hakları Yön. de "bir suretini alma hakkı" verir. |
| Düzeltme (rectification) | KVKK m. 11/d | Observation `amended` durumu zaten şemada var; orijinal değer ve gerekçe `audit_log.metadata`'da. **Hard-delete asla.** |
| Silme/yok etme | KVKK m. 11/e | Karmaşık — §13. Hekimin saklama yükümlülüğü ile çatışır. |
| Aktarılan üçüncü kişileri öğrenme | KVKK m. 11/c | `audit_log`'dan üretilebilir rapor: hangi observation kimlere export edildi. |
| Otomatik analize itiraz | KVKK m. 11/g | "Bu hasta için LLM analizini devre dışı bırak" hasta-bazlı flag. |
| Zarar tazmin | KVKK m. 11/h | Yazılımda karşılığı yok — hukuki süreç. |

**Hasta Hakları Yönetmeliği ek hakları:**
- Aydınlatılmış onam (m. 22) — anamnez'in audit ettiği şey değil ama klinik içi süreç. UI'da onam alındığını işaretleyen bir bayrak makul.
- Mahremiyet (m. 21) — patient_access ACL bunu karşılıyor.
- Hasta dosyasına erişim (m. 42) — PDF export.

**Üçüncü kişi (avukat, yakın) talepleri:**
- Avukat: KSV Yönetmeliği m. 10 — genel vekâletname yetmez, **özel açık rıza** beyanı şart. Anamnez UI'sında "bu kayıtları hastanın avukatına aktarıyorum" akışı, özel rıza belgesinin yüklenmesini zorunlu kılmalı.
- Yakın: Hasta Hakları Yönetmeliği m. 18 — yetişkin hastada yakına bilgilendirme **hastanın izniyle**.
- Veli/vasi (reşit olmayan, vesayet altındakiler): KSV Yön. 2025 değişikliğiyle "bakım veren kişi" tanımı geldi — boşanmış velâyetsiz ebeveynin erişimi sınırlı. Anamnez bunu doğrudan kontrol edemez (klinik karar verir); ama erişimin sebebi `audit_log`'a düşmeli.

**Ölü kişilerin verileri:** KSV Yön. m. 11 — yasal mirasçı veraset ilamı ile talep edebilir. Saklama süresi **en az 20 yıl**. Anamnez retention politikası (§13) bu süreyi ihlal edemez.

## 13. Saklama ve imha — Silme/İmha Yönetmeliği

VERBİS kaydı yapan veri sorumluları **Kişisel Veri Saklama ve İmha Politikası** hazırlamak zorunda. Muaf olanlar için politika zorunlu değil ama imha yükümlülüğü vardır (3 ay içinde, periyodik imha 6 ay aralıkla maks.).

**Anamnez retention politikası — varsayılan değerler (klinik UI'dan değiştirilebilir):**

| Veri türü | Saklama süresi | Gerekçe |
|---|---|---|
| Observation, source document, extraction | **20 yıl** (son işlemden veya hasta ölümünden itibaren) | KSV Yön. m. 11 atfı + hekimlik klinik kayıt standardı. |
| Audit log | 10 yıl | §5. |
| `session` | Expiry + 90 gün (forensic) sonra silme | Aktif session değil — login geçmişi audit'te zaten. |
| `user` (disabled) | Hesap devre dışı bırakıldıktan 10 yıl sonra fiziksel silme | Audit referansları kalmalı — `actor_user_id` orphan değil. Yumuşak silme + retention süresi sonu hard delete + audit log'da silme kaydı. |
| Test verisi (`ENV=TEST` üretti) | Her gece tamamen sil | DB temizliği. |
| Yedekleme dosyaları | 1 yıl rolling (52 haftalık + 12 aylık) | Dengeli — felaket kurtarma vs. minimizasyon. |

**Periyodik imha**: gece bir job, 6 aydan eski "silinmesi gereken" satırları yok eder. Her imha turu kendi audit kaydını üretir (Silme/İmha Yönetmeliği m. 7 — kayıt en az 3 yıl).

**Silme talebi geldiğinde** (m. 11/e): klinik onaylar — çünkü hekimin **tıbbi kayıt saklama yükümlülüğü 20 yıl** olabilir ve bu KVKK silme hakkını sınırlandırır (KVKK m. 28 ve hukuki yükümlülükler). Anamnez:
1. Talep işlemini bir admin UI iş akışına dönüştürür.
2. Admin "şu hastanın şu kategorideki verileri silinsin" derse, silme yerine **şartlı silme (suppression)** uygular — veri kullanılamaz hale getirilir ama yasal saklama süresi dolana kadar fiziksel olarak durur. Süre dolunca otomatik hard delete + audit kaydı.
3. Anonimleştirme alternatifi UI'da sunulur — anonim hale getirilen veri Silme/İmha Yönetmeliği m. 8 anlamında "silinmiş" sayılır ve aynı zamanda bilimsel kullanım için elverişli kalır.

## 14. Veri ihlali bildirimi — m. 12/5 + 2019/10

İhlali öğrendiği tarihten itibaren **gecikmeksizin ve en geç 72 saat** içinde Kurul'a bildirim şart (2019/10). Etkilenen ilgili kişilere de **makul süre içinde** bildirim.

Anamnez'in olay müdahale (incident response) desteği:

- **Tespit**: audit_log üzerinde basit anomali kuralları — anormal yüksek görüntüleme hacmi, normalde erişimi olmayan kullanıcının erişimi, çok sayıda başarısız login. MVP'de manuel inceleme yeterli, ama admin dashboard'da "şüpheli aktivite" widget'ı.
- **Kapsam çıkarımı**: bir saldırgan veya çalınmış oturum X bilindiğinde, anamnez "bu session/kullanıcı tarafından yapılan tüm erişimler" raporunu üretebilmeli — hangi hastalar etkilendi, hangi observation'lar görüldü/değiştirildi. `audit_log.session_id` ve `patient_id` denormalize alanları bunu mümkün kılıyor (zaten README'de).
- **Bildirim şablonu**: KVKK web sitesindeki "Kişisel Veri İhlali Bildirim Formu" doldurulması gereken alanları (ihlal tarihi, etkilenen veri kategorileri, etkilenen kişi sayısı, olası sonuçlar, alınan önlemler) anamnez bir raporda derleyebilir.
- **Workstation kaybı/çalınması**: enrollment fingerprint sistemi sayesinde admin UI'dan workstation revoke edilir; o tarihten sonra o cihazdan gelen istek reddedilir. Bu olayın kendisi bildirim gerektirmez (cihazda hasta verisi yerel olarak saklanmıyorsa — bunu doğrulayın: workstation client local cache yapmasın veya yaparsa şifreli ve revoke ile birlikte silinebilir olsun).

## 15. Ceza riskleri — sayılarla

| İhlal | 2026 idari para cezası aralığı |
|---|---|
| Aydınlatma yükümlülüğüne aykırılık (m. 18/1-a) | ~106.250 TL – 2.125.000 TL |
| Veri güvenliği yükümlülüğüne aykırılık (m. 18/1-b) | ~318.750 TL – 21.250.000 TL |
| Kurul kararına aykırılık (m. 18/1-c) | ~531.250 TL – 21.250.000 TL |
| VERBİS yükümlülüğüne aykırılık (m. 18/1-ç) | ~425.000 TL – 21.250.000 TL |
| Yurt dışı aktarım sözleşme bildiriminin yapılmaması (yeni m. 18/1-d) | ~90.000 TL – 1.800.000 TL |

(Rakamlar 2026 yeniden değerleme katsayısına göre yaklaşıktır; her takvim yılında Kurum güncel listeyi yayımlar — kesin rakam için yıllık duyuruya bakın.)

Ek olarak:
- **TCK 136**: kişisel veriyi hukuka aykırı şekilde başkasına verme — 2-4 yıl hapis.
- **TCK 137**: suç bir kamu görevlisi tarafından veya bir mesleğin sağladığı kolaylıktan yararlanmak suretiyle işlenirse ceza yarı oranında artırılır — hekimler bu kapsamdadır.

## 16. Klinik yükümlülükleri (yazılım dışı)

Anamnez bunları üretemez; klinik üretmek zorundadır. Onboarding sırasında klinik adminine bir checklist sunulmalı:

- [ ] Klinik için **Kişisel Veri İşleme Envanteri**.
- [ ] **Aydınlatma metni** — anamnez şablonundan klinik bilgilerine göre uyarlanmış (§7).
- [ ] Personel için **gizlilik/sır saklama sözleşmesi**.
- [ ] Personel için **KVKK ve sır saklama eğitimi** kaydı.
- [ ] Eşikler geçildiyse **VERBİS kaydı** (§9).
- [ ] **Kişisel Veri Saklama ve İmha Politikası** belgesi (VERBİS kaydı varsa zorunlu).
- [ ] **İhlal müdahale planı** — kim arayacak, kim Kurum'a bildirecek, hangi telefonla.
- [ ] Mac Studio için **fiziksel güvenlik** (kilitli alan, yetkili erişim).
- [ ] Yedekleme medyası için fiziksel güvenlik (§6).
- [ ] **İlgili kişi başvuru kanalı** — yazılı veya KEP adresi.
- [ ] Anamnez ile **DPA / tedarik sözleşmesi**.

## 17. MVP için yazılım iş listesi — KVKK türevi

`README.md` zaten çoğu güvenliği taşıyor. KVKK'nın açıkça eklediği iş kalemleri:

1. **SQLCipher veya equivalent at-rest encryption**; anahtar Secure Enclave'de.
2. **Otomatik şifreli yedekleme**; anahtar yönetimi belgelendi.
3. **`Environment::Test` enum'u + üretim DB marker'ı** — `ENV=TEST` binary üretim DB'sini açamasın (§10).
4. **Audit log retention job** — 10 yıl + her gece eski kayıtları temizle.
5. **Periyodik imha job** — observation/source document retention politikasına göre.
6. **Suppression desteği** — silme talebi için fiziksel silme yerine erişimi engelle, saklama süresi sonu hard delete.
7. **Hasta dosyası PDF export** — m. 11/b + Hasta Hakları Yön. m. 42.
8. **İhlal kapsam raporu** — `(session_id, user_id, time_range)` parametresiyle etkilenen hasta/observation listesi.
9. **İşleme envanteri raporu** — VERBİS için klinik kullanabilsin (§9).
10. **Aydınlatma metni şablonu** + klinik bilgisi ile doldurulup PDF üretebilen admin sayfası.
11. **Onam takibi tablosu** — özel rıza gerektiren akışlar için (avukata aktarım, anonim olmayan araştırma kullanımı).
12. **`extracted_by = 'llm'` görsel ayrım** — UI tüm AI çıkarımlarını "hekim onayı bekliyor" şeklinde göstersin.
13. **Workstation revoke** + workstation client'ın hiç **persistent hasta verisi cache'lememesi**.
14. **Anomali widget'ı** — admin dashboard'da 7 günlük şüpheli aktivite özeti.
15. **Periyodik yetki gözden geçirme** raporu — 6 ay erişim kullanmayanlar.

## 18. Açık konular (ileride netleştirilecek)

- **e-Nabız entegrasyonu**: KSV Yönetmeliği'nin 2025 değişikliği e-Nabız'ı genişçe varsayıyor. Anamnez MVP'de e-Nabız ile konuşmuyor; ileride hekimin hastanın geçmiş Bakanlık kayıtlarını çekmesi istenirse, Sağlık Bakanlığı API protokolü ve ek aydınlatma şart.
- **İki faktörlü kimlik doğrulama — pozisyonumuz**: 2018/10'un MFA cümlesi spesifik olarak **uzak erişim** içindir; MVP'de uzak erişim mimariyle reddediliyor, bu yüzden bu cümlenin lafzı tetiklenmiyor. Yerel erişim için pozisyonumuz: workstation'ın enrolled device credential'ı = "sahip olduğun şey", kullanıcı şifresi = "bildiğin şey" — toplamda iki faktör; m. 12'nin "yeterli teknik tedbir" ölçüsünü karşılar. Bu pozisyon ancak şu varsayımlar gerçekten doğruysa ayakta durur: cihaz tek bir kullanıcıya bağlı, oturum boşta kilitleniyor, yüksek riskli admin işlemleri step-up doğrulama gerektiriyor, ve bu çerçeve klinik güvenlik politikasında + DPA'da yazılı. Bu bağımlılıkların tümü `KVKK-suggestions.md`'de listelendi ve README'ye dahil edilmeli — onlar girmedikçe pozisyon savunulabilir değildir.
- **Sigorta**: Siber sigorta KVKK için zorunlu değil ama Kurulu cezalarını sermaye olarak tampone etmek için klinik tarafından düşünülmeli.
- **DPO**: KVKK zorunlu DPO'su yok (henüz). GDPR'da var. Klinik gönüllü olarak "irtibat kişisi" atayabilir.
