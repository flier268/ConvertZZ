using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;

namespace ConvertZZ.Moudle
{
    public class ConvertHelper
    {
        /// <summary>
        /// 轉換文字
        /// </summary>
        /// <param name="origin">原始文字</param>
        /// <param name="ToChinese">0: 不轉換 1: 簡體轉繁體 2:繁體轉簡體</param>
        /// <param name="VocabularyCorrection">-1: 依照設定值變動 0:不使用辭典轉換 1:使用辭典轉換</param>
        /// <returns></returns>
        public static string Convert(string origin, int ToChinese, int VocabularyCorrection = -1)
        {
            if (String.IsNullOrWhiteSpace(origin)) return origin;
            if (!App.DicLoaded)
            {
                System.Threading.SpinWait.SpinUntil(() => App.DicLoaded, 10000);
                if (!App.DicLoaded)
                    throw new Exception("詞彙修正的Dictionary載入失敗");
            }
            switch (ToChinese)
            {
                case 1:
                    if ((App.Settings.VocabularyCorrection && VocabularyCorrection != 0) || VocabularyCorrection == 1)
                    {
                        origin = App.ChineseConverter.Convert(origin, true);
                    }
                    origin = ChineseConverter.ToTraditional(origin);
                    break;
                case 2:
                    if ((App.Settings.VocabularyCorrection && VocabularyCorrection != 0) || VocabularyCorrection == 1)
                    {
                        origin = App.ChineseConverter.Convert(origin, false);
                    }
                    origin = ChineseConverter.ToSimplified(origin);
                    break;
            }
            return origin;
        }
        /// <summary>
        /// 轉換文字
        /// </summary>
        /// <param name="origin">原始文字</param>
        /// <param name="encoding">encoding[0]:來源編碼  encoding[1]:目標編碼</param>
        /// <param name="ToChinese">0: 不轉換 1: 簡體轉繁體 2:繁體轉簡體</param>
        /// <param name="VocabularyCorrection">-1: 依照設定值變動 0:不使用辭典轉換 1:使用辭典轉換</param>
        /// <returns></returns>
        public static string Convert(string origin, Encoding[] encoding, int ToChinese, int VocabularyCorrection = -1)
        {
            if (String.IsNullOrWhiteSpace(origin)) return origin;
            switch (ToChinese)
            {
                case 1:
                    return Convert(encoding[0].GetString(encoding[1].GetBytes(Convert(origin, 0, VocabularyCorrection))),ToChinese);
                case 2:                    
                default:
                    return encoding[0].GetString(encoding[1].GetBytes(Convert(origin, ToChinese, VocabularyCorrection)));
            }
        }

        public static string FileConvert(string origin, Encoding[] encoding, int ToChinese, int VocabularyCorrection = -1)
        {
            if (ToChinese == 0)
            {
                //經研究，ConvertZ在轉檔案時，做了一些小動作，他會先把原本big5顯示不出來的字轉成繁體，證據是'软'都變成'軟'了
                StringBuilder sb = new StringBuilder(origin.Length);
                foreach (char c in origin.ToCharArray())
                    if (encoding[1].GetChars(encoding[1].GetBytes(new char[] { c }))[0] != c)
                    {
                        sb.Append(ChineseConverter.ToTraditional(new String(c, 1)));
                    }
                    else
                        sb.Append(c);
                origin = sb.ToString();
            }
            if (encoding[1] == Encoding.Default || encoding[1] == Encoding.UTF8 || encoding[1] == Encoding.Unicode || encoding[1] == Encoding.GetEncoding("UnicodeFFFE") || encoding[0] == encoding[1])
                return Convert(origin, ToChinese, VocabularyCorrection);
            else
                return Convert(origin, new Encoding[2] { Encoding.Default, encoding[1] }, ToChinese, VocabularyCorrection);
        }

        /// <summary>
        /// 進行標點符號轉換
        /// </summary>
        /// <param name="origin">來源字串</param>
        /// <param name="mode">0: 半形轉全形 ; 1:全形轉半形</param>
        /// <returns></returns>
        public static string ConvertSymbol(string origin, int mode)
        {   
            FastReplace fastReplace = new FastReplace(mode == 0 ? SymbolTable : (SymbolTable.ToLookup(pair => pair.Value, pair => pair.Key).ToDictionary(grp => grp.Key, grp => grp.ToArray()[0])));
            return fastReplace.ReplaceAll(origin);
        }
        private static Dictionary<string, string> SymbolTable = new Dictionary<string, string>()
        {
            { "," , "，" },
            { "~" , "～" },
            { "!" , "！" },
            { "#" , "＃" },
            { "$" , "＄" },
            { "%" , "％" },
            { "^" , "︿" },
            { "&" , "＆" },
            { "*" , "＊" },
            { "-" , "－" },
            { "+" , "＋" },
            { "{" , "｛" },
            { "}" , "｝" },
            { ";" , "；" },
            { "|" , "｜" },
            { "?" , "？" },
            { "(" , "（" },
            { ")" , "）" },
            { "“" , "「" },
            { "”" , "」" },
            { "‘" , "『" },
            { "’" , "』" },
            { "[" , "［" },
            { "]" , "］" },
            //{ "·" , "．" },
            { " " , "　" },

            { ":" , "：" },
            { "." , "。" },
            { "\"" , "、" },
            { "@" , "＠" },
            { "<" , "＜" },
            { ">" , "＞" },
            { "=" , "＝" },
        };
    }
}
