using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace ConvertZZ.Moudle
{
    public class ConvertHelper
    {
        public static string Convert(string origin,Encoding[] encoding,int ToChinese)
        {
            switch (ToChinese)
            {
                case 1:
                    origin = ChineseConverter.ToTraditional(origin);
                    if (App.Settings.VocabularyCorrection)
                        origin = App.ChineseConverter.Convert(origin);
                    break;
                case 2:
                    origin = ChineseConverter.ToSimplified(origin);
                    if (App.Settings.VocabularyCorrection)
                        origin = App.ChineseConverter.Convert(origin);
                    break;
            }
            return origin == null ? null : encoding[1].GetString(encoding[0].GetBytes(origin));
        }
    }
}
