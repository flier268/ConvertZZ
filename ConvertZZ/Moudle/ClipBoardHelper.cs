using System;
using System.Threading;
using System.Windows;

namespace ConvertZZ
{
    public class ClipBoardHelper
    {
        public static string GetClipBoard()
        {
            string idat = null;
            Exception threadEx = null;
            Thread staThread = new Thread(
                delegate ()
                {
                    try
                    {
                        idat = Clipboard.GetText(TextDataFormat.UnicodeText);
                    }

                    catch (Exception ex)
                    {
                        threadEx = ex;
                    }
                });
            staThread.SetApartmentState(ApartmentState.STA);
            staThread.Start();
            staThread.Join();
            return idat;
        }
        public static void SetClipBoard(string str)
        {
            Exception threadEx = null;
            Thread staThread = new Thread(
                delegate ()
                {
                    try
                    {
                        Clipboard.SetText(str, TextDataFormat.UnicodeText);
                    }

                    catch (Exception ex)
                    {
                        threadEx = ex;
                    }
                });
            staThread.SetApartmentState(ApartmentState.STA);
            staThread.Start();
            staThread.Join();
        }
    }
}
