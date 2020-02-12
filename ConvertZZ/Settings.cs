// To parse this JSON data, add NuGet 'Newtonsoft.Json' then do:
//
//    using ConvertZZ;
//
//    var settings = Settings.FromJson(jsonString);

namespace ConvertZZ
{
    using ConvertZZ.Class;
    using ConvertZZ.Enums;
    using ConvertZZ.Moudle;
    using Newtonsoft.Json;
    using Newtonsoft.Json.Converters;
    using System.Collections.Generic;
    using System.Collections.ObjectModel;
    using System.Globalization;
    using System.IO;
    using System.Linq;
    using System.Text;

    public partial class App
    {
        static string _FilePath;
        public static Settings Settings { get; set; } = new Settings();
        public static void Reload(string FilePath)
        {
            _FilePath = FilePath;
            if (File.Exists(FilePath))
            {
                using (StreamReader streamReader = new StreamReader(FilePath, Encoding.UTF8))
                {
                    var temp = Settings.FromJson(streamReader.ReadToEnd());
                    if (!temp.FileConvert.TypeFilter.Contains("<"))
                        temp.FileConvert.TypeFilter = new Settings().FileConvert.TypeFilter;
                    Settings = temp;
                }
            }
        }
        public static void Save()
        {
            using (StreamWriter sw = new StreamWriter(_FilePath, false, Encoding.UTF8))
            {
                sw.Write(Settings.ToJson());
                sw.Flush();
            }
        }
    }


    public partial class Settings
    {
        public Settings()
        {
            PositionX = -1;
            PositionY = -1;
            QuickStart = new QuickStart();
            RecognitionEncoding = true;
            Prompt = true;
            MaxLengthPreview = 6;
            VocabularyCorrection = true;
            AssistiveTouch = true;
            HotKey = new HotKey();
            FileConvert = new FileConvert();
            CheckVersion = true;
            Engine = Enum_Engine.Local;
            Fanhuaji_Setting = new Fanhuaji_Config();
        }
        /// <summary>
        /// 上次關閉時的X座標
        /// </summary>
        [JsonProperty("PositionX")]
        public double PositionX { get; set; }
        /// <summary>
        /// 上次關閉時的Y座標
        /// </summary>
        [JsonProperty("PositionY")]
        public double PositionY { get; set; }
        /// <summary>
        /// 試圖自動辨識編碼
        /// </summary>
        [JsonProperty("RecognitionEncoding")]
        public bool RecognitionEncoding { get; set; }
        /// <summary>
        /// 轉換完成後做出提示
        /// </summary>
        [JsonProperty("Prompt")]
        public bool Prompt { get; set; }
        /// <summary>
        /// 預覽的最大長度(kb)
        /// </summary>
        [JsonProperty("MaxLengthPreview")]
        public int MaxLengthPreview { get; set; }
        /// <summary>
        /// 詞彙修正
        /// </summary>
        [JsonProperty("Vocabulary correction")]
        public bool VocabularyCorrection { get; set; }
        /// <summary>
        /// 詞彙修正引擎
        /// </summary>
        [JsonProperty("Engine")]
        public Enum_Engine Engine { get; set; }
        /// <summary>
        /// 啟用懸浮球
        /// </summary>
        [JsonProperty("AssistiveTouch")]
        public bool AssistiveTouch { get; set; }
        /// <summary>
        /// 快速動作設定(輔助鍵+點擊)
        /// </summary>
        [JsonProperty("QuickStart")]
        public QuickStart QuickStart { get; set; }
        /// <summary>
        /// 快捷鍵
        /// </summary>
        [JsonProperty("HotKey")]
        public HotKey HotKey { get; set; }
        /// <summary>
        /// 檔案轉換
        /// </summary>
        [JsonProperty("FileConvert")]
        public FileConvert FileConvert { get; set; }
        /// <summary>
        /// 繁化姬設定檔
        /// </summary>
        [JsonProperty("Fanhuaji_Setting")]
        public Fanhuaji_Config Fanhuaji_Setting { get; set; }
        /// <summary>
        /// 啟動時檢查更新
        /// </summary>
        [JsonProperty("CheckVersion")]
        public bool CheckVersion { get; set; }
    }

    public partial class FileConvert
    {
        public FileConvert()
        {
            UnicodeAddBom = false;
            DefaultPath = "!";
            TypeFilter = "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音頻文件|*.mp3>";
            FixLabel = ".htm|.html|.shtm|.shtml|.asp|.apsx|.php";
        }
        /// <summary>
        /// 預設路徑
        /// </summary>
        [JsonProperty("DefaultPath")]
        public string DefaultPath { get; set; }
        /// <summary>
        /// 類型篩選器
        /// </summary>
        [JsonProperty("TypeFilter")]
        public string TypeFilter { get; set; }
        /// <summary>
        /// 修正檔案內文的編碼標籤
        /// </summary>
        [JsonProperty("FixLabel")]
        public string FixLabel { get; set; }
        /// <summary>
        /// 加入BOM到Unicode檔頭
        /// </summary>
        [JsonProperty("UnicodeAddBOM")]
        public bool UnicodeAddBom { get; set; }

        /// <summary>
        /// 取得副檔名篩選器清單
        /// </summary>
        /// <returns></returns>
        public List<string> GetFilterList(bool AddFixedItem = true)
        {
            List<string> filter = new List<string>();
            if (AddFixedItem)
                filter.Add("任意檔案(*.*)|*.*");
            System.Text.RegularExpressions.Regex r_filter = new System.Text.RegularExpressions.Regex("<(.*?)>");
            TypeFilter.Split('/').ToList().ForEach(x =>
            {
                filter.Add(r_filter.Match(x).Groups[1].Value);
            });
            return filter;
        }
        /// <summary>
        /// 傳回(.*?)|(.*?)
        /// </summary>
        /// <param name="Filter"></param>
        /// <returns></returns>
        public string[] SplitFilterString(string Filter)
        {
            var r = System.Text.RegularExpressions.Regex.Match(Filter, @"(.*?)\|(.*?)$");
            return new string[] { r.Groups[1].Value, r.Groups[2].Value };
        }
        /// <summary>
        /// 傳回用';'分割的List
        /// </summary>
        /// <param name="str"></param>
        /// <returns></returns>
        public List<string> GetExtentionArray(string str)
        {
            return SplitFilterString(str)[1].Split(';').ToList();
        }
        /// <summary>
        /// 檢查filename是否符合正則表達式extension
        /// </summary>
        /// <param name="filename"></param>
        /// <param name="extension"></param>
        /// <returns></returns>
        public bool CheckExtension(string filename, string extension)
        {
            extension = extension.Trim().Replace(".", "||||||||").Replace("*", ".*?").Replace("||||||||", @"\.") + "$";
            return System.Text.RegularExpressions.Regex.IsMatch(filename, extension);
        }

        public void CallFilterEditor()
        {
            var temp = new ObservableCollection<Window_KeyValueEditor.KeyValueItem>();
            App.Settings.FileConvert.GetFilterList(false).ForEach(x =>
            {
                var _t = App.Settings.FileConvert.SplitFilterString(x);
                temp.Add(new Window_KeyValueEditor.KeyValueItem() { Key = _t[0], Value = _t[1] });
            });
            Window_KeyValueEditor window_KeyValueEditor = new Window_KeyValueEditor(
                new Window_KeyValueEditor.Button() { Content = "Save" },
                new Window_KeyValueEditor.Button() { Content = "Close" },
                temp
                );
            window_KeyValueEditor.Button2_Action = new System.Action(() =>
            {
                window_KeyValueEditor.Close();
            });
            window_KeyValueEditor.Button1_Action = new System.Action(() =>
            {
                StringBuilder sb = new StringBuilder();
                window_KeyValueEditor.KeyValueItems.ToList().ForEach(x =>
                {
                    if (sb.Length > 0)
                        sb.Append("/");
                    sb.AppendFormat("<{0}|{1}>", x.Key, x.Value);
                });
                TypeFilter = sb.ToString();
                App.Save();
                window_KeyValueEditor.Close();
            });
            window_KeyValueEditor.ShowDialog();
        }
    }

    public partial class HotKey
    {
        public HotKey()
        {
            AutoCopy = true;
            AutoPaste = true;
            Feature1 = new Feature() { Action = "a1", Enable = false, Key = "None", Modift = "None" };
            Feature2 = new Feature() { Action = "a2", Enable = false, Key = "None", Modift = "None" };
            Feature3 = new Feature() { Action = "a3", Enable = false, Key = "None", Modift = "None" };
            Feature4 = new Feature() { Action = "a4", Enable = false, Key = "None", Modift = "None" };
        }
        [JsonProperty("AutoCopy")]
        public bool AutoCopy { get; set; }

        [JsonProperty("AutoPaste")]
        public bool AutoPaste { get; set; }

        [JsonProperty("Feature1")]
        public Feature Feature1 { get; set; }

        [JsonProperty("Feature2")]
        public Feature Feature2 { get; set; }

        [JsonProperty("Feature3")]
        public Feature Feature3 { get; set; }

        [JsonProperty("Feature4")]
        public Feature Feature4 { get; set; }
    }

    public partial class Feature
    {
        public Feature()
        {
            Action = "";
            Enable = false;
            Key = "";
            Modift = "";
        }
        [JsonProperty("Action")]
        public string Action { get; set; }

        [JsonProperty("Enable")]
        public bool Enable { get; set; }

        [JsonProperty("Key")]
        public string Key { get; set; }

        [JsonProperty("Modift")]
        public string Modift { get; set; }
    }

    public partial class QuickStart
    {
        public QuickStart()
        {
            LeftClick_Ctrl = "";
            LeftClick_Alt = "";
            LeftClick_Shift = "";
            RightClick_Ctrl = "";
            RightClick_Alt = "";
            RightClick_Shift = "";
            LeftDrop_Ctrl = "";
            LeftDrop_Alt = "";
            LeftDrop_Shift = "";
            RightDrop_Ctrl = "";
            RightDrop_Alt = "";
            RightDrop_Shift = "";
        }
        [JsonProperty("LeftClick_Ctrl")]
        public string LeftClick_Ctrl { get; set; }
        [JsonProperty("LeftClick_Alt")]
        public string LeftClick_Alt { get; set; }
        [JsonProperty("LeftClick_Shift")]
        public string LeftClick_Shift { get; set; }
        [JsonProperty("RightClick_Ctrl")]
        public string RightClick_Ctrl { get; set; }
        [JsonProperty("RightClick_Alt")]
        public string RightClick_Alt { get; set; }
        [JsonProperty("RightClick_Shift")]
        public string RightClick_Shift { get; set; }
        [JsonProperty("LeftDrop_Ctrl")]
        public string LeftDrop_Ctrl { get; set; }
        [JsonProperty("LeftDrop_Alt")]
        public string LeftDrop_Alt { get; set; }
        [JsonProperty("LeftDrop_Shift")]
        public string LeftDrop_Shift { get; set; }
        [JsonProperty("RightDrop_Ctrl")]
        public string RightDrop_Ctrl { get; set; }
        [JsonProperty("RightDrop_Alt")]
        public string RightDrop_Alt { get; set; }
        [JsonProperty("RightDrop_Shift")]
        public string RightDrop_Shift { get; set; }
    }

    public partial class Settings
    {
        public static Settings FromJson(string json) => JsonConvert.DeserializeObject<Settings>(json, ConvertZZ.Converter.Settings);
    }

    public static class Serialize
    {
        public static string ToJson(this Settings self) => JsonConvert.SerializeObject(self, Formatting.Indented, ConvertZZ.Converter.Settings);
    }

    internal class Converter
    {
        public static readonly JsonSerializerSettings Settings = new JsonSerializerSettings
        {
            MetadataPropertyHandling = MetadataPropertyHandling.Ignore,
            DateParseHandling = DateParseHandling.None,
            Converters = {
                new IsoDateTimeConverter { DateTimeStyles = DateTimeStyles.AssumeUniversal }
            },
        };
        public static readonly JsonSerializerSettings Config = new JsonSerializerSettings
        {
            MetadataPropertyHandling = MetadataPropertyHandling.Ignore,
            DateParseHandling = DateParseHandling.None
        };
    }
}
