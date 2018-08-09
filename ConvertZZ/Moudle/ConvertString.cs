using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ.Moudle
{
    public class ConvertHelper
    {
        public static string Convert(string origin, int ToChinese)
        {
            if (String.IsNullOrWhiteSpace(origin)) return origin;
            switch (ToChinese)
            {
                case 1:
                    if (App.Settings.VocabularyCorrection)
                    {
                        System.Threading.SpinWait.SpinUntil(() => App.DicLoaded, 3000);
                        if (!App.DicLoaded)
                            throw new Exception("詞彙修正的Dictionary載入失敗");
                        origin = App.ChineseConverter.Convert(origin);
                    }
                    origin = ChineseConverter.ToTraditional(origin);
                    break;
                case 2:
                    /*
                    if (App.Settings.VocabularyCorrection)
                        origin = App.ChineseConverter.Convert(origin);*/
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
    }
}
