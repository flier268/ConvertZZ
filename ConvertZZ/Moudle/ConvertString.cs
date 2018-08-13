using System;
using System.Text;

namespace ConvertZZ.Moudle
{
    public class ConvertHelper
    {
        public static string Convert(string origin, int ToChinese)
        {
            if (String.IsNullOrWhiteSpace(origin)) return origin;
            if (!App.DicLoaded)
            {
                System.Threading.SpinWait.SpinUntil(() => App.DicLoaded, 3000);
                if (!App.DicLoaded)
                    throw new Exception("詞彙修正的Dictionary載入失敗");
            }
            switch (ToChinese)
            {
                case 1:
                    if (App.Settings.VocabularyCorrection)
                    {
                        origin = App.ChineseConverter.Convert(origin, true);
                    }
                    origin = ChineseConverter.ToTraditional(origin);
                    break;
                case 2:
                    if (App.Settings.VocabularyCorrection)
                    {
                        origin = App.ChineseConverter.Convert(origin, false);
                    }
                    origin = ChineseConverter.ToSimplified(origin);
                    break;
            }
            return origin;
        }
        public static string Convert(string origin, Encoding[] encoding, int ToChinese)
        {
            if (String.IsNullOrWhiteSpace(origin)) return origin;
            return encoding[0].GetString(encoding[1].GetBytes(Convert(origin, ToChinese)));
        }

        public static string FileConvert(string origin, Encoding[] encoding, int ToChinese)
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
                return Convert(origin, ToChinese);
            else
                return Convert(origin, new Encoding[2] { Encoding.Default, encoding[1] }, ToChinese);
        }
    }
}
